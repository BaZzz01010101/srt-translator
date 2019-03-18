use std::collections::hash_map::HashMap;
use std::fs::File;
use std::io::{Read, Write, Result};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

use chrono::{Utc, NaiveTime};
use clap::{App, Arg};
use regex::Regex;
use regex::Captures;
use translate_core::*;
use std::fmt;

struct Args {
  input_subs_filename: String,
  output_subs_filename: String,
  database_filename: String,
  analyze_mode: bool,
}

struct Sub {
  index: u32,
  start_time: NaiveTime,
  end_time: NaiveTime,
  text: String,
  need_translation: bool,
}

enum WordKind {
  Known,
  Unknown,
  New,
}

impl FromStr for WordKind {
  type Err = String;

  fn from_str(s: &str) -> std::result::Result<WordKind, Self::Err> {
    match s {
      "k" => Ok(WordKind::Known),
      "u" => Ok(WordKind::Unknown),
      "?" => Ok(WordKind::New),
      _ => Err(String::from("Parsing error"))
    }
  }
}

struct Word<'a> {
  text: &'a str,
  kind: WordKind,
}

impl fmt::Display for Sub {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}\n{} --> {}\n{}\n",
           self.index,
           self.start_time.format("%H:%M:%S,%3f"),
           self.end_time.format("%H:%M:%S,%3f"),
           self.text)
  }
}

impl Sub {
  fn stringify(&self) -> String {
    format!("{}\n{} --> {}\n{}\n\n",
            self.index,
            self.start_time.format("%H:%M:%S,%3f"),
            self.end_time.format("%H:%M:%S,%3f"),
            self.text)
  }
}

fn get_args() -> Args {
  let matches = App::new("Word Parser")
    .version("1.0")
    .author("ZeuS <andy2002ua@gmail.com>")
    .about("Translate given subtitles file selectively using lists of known and unknown words")
    .arg(Arg::with_name("input")
      .required(true)
      .value_name("INPUT SUBS")
      .help("Sets an input subtitles file")
      .index(1))
    .arg(Arg::with_name("output")
      .short("o")
      .long("output-subs")
      .value_name("OUTPUT SUBS")
      .takes_value(true)
      .help("Sets the output subtitles file")
      .index(2))
    .arg(Arg::with_name("database")
      .short("d")
      .long("database-file")
      .value_name("DATABASE FILE")
      .takes_value(true)
      .help("Sets the database file"))
    .arg(Arg::with_name("analyze")
      .short("a")
      .long("analyze")
      .help("Skip translation and feel words database"))
    .get_matches();

  let input_subs_filename = matches.value_of("input").unwrap().to_owned();
  let mut input_file_path;

  let output_subs_filename = match matches.value_of("output") {
    Some(name) => name.to_owned(),
    None => {
      input_file_path = PathBuf::from(&input_subs_filename);
      input_file_path.set_extension("out.srt");
      input_file_path.to_str().unwrap().to_owned()
    }
  };

  let database_filename = match matches.value_of("database") {
    Some(name) => name.to_owned(),
    None => {
      let mut filename = std::env::current_exe().unwrap();
      filename.set_file_name("words.db");
      String::from(filename.to_str().unwrap())
    }
  };

  let analyze_mode = matches.is_present("analyze");

  Args {
    input_subs_filename,
    output_subs_filename,
    database_filename,
    analyze_mode,
  }
}

fn load_text_file<P>(file_name: P) -> Result<String> where P: AsRef<Path> {
  let mut text = String::new();
  let mut input_file = File::open(file_name)?;
  input_file.read_to_string(&mut text)?;

  Ok(text)
}

fn parse_subs(text: &String) -> Vec<Sub> {
  let mut subs = Vec::new();

  let re = Regex::new(r"(?msx)
        (?P<index>\d+)\r?\n
        (?P<start_time>\d{2}:\d{2}:\d{2},\d{3})\s-->\s(?P<end_time>\d{2}:\d{2}:\d{2},\d{3})\r?\n
        (?P<text>.+?)\r?\n\r?\n
    ").unwrap();

  for caps in re.captures_iter(text.as_str()) {
    let index: u32 = caps.name("index").unwrap().as_str().parse().unwrap();
    let start_time = NaiveTime::parse_from_str(caps.name("start_time").unwrap().as_str(), "%H:%M:%S,%3f").unwrap();
    let end_time = NaiveTime::parse_from_str(caps.name("end_time").unwrap().as_str(), "%H:%M:%S,%3f").unwrap();
    let text = caps.name("text").unwrap().as_str().to_owned();

    subs.push(Sub {
      index,
      start_time,
      end_time,
      text,
      need_translation: false,
    });
  }

  subs
}

fn parse_db_words(text: &String) -> HashMap<&str, Word> {
  let mut words = HashMap::new();
  // TODO: make 're' const
  let re = Regex::new(r"(?P<type>[\?ku]):(?P<text>.+?)\r?\n").unwrap();

  // TODO: replace by functional 'map' if possible
  for caps in re.captures_iter(text.as_str()) {
    let kind: WordKind = caps.name("type").unwrap().as_str().parse().unwrap();
    let text = caps.name("text").unwrap().as_str();

    words.insert(text, Word {
      text,
      kind,
    });
  }

  words
}

fn parse_sub_words(lowercase_subs_text: &String) -> HashMap<&str, Word> {
  let mut sub_words: HashMap<&str, Word> = HashMap::new();

  let re = Regex::new(r"(?msx)(?:(?P<word>[a-z']+?)[^a-z']+)").unwrap();

  for caps in re.captures_iter(lowercase_subs_text.as_str()) {
    let text = caps.name("word").unwrap().as_str();

    sub_words.insert(text, Word {
      text,
      kind: WordKind::New,
    });
  }

  sub_words
}

fn translate_subs(subs: &mut Vec<Sub>, words: &HashMap<&str, Word>) {
  let re_color = Regex::new("([a-zA-Z'])+").unwrap();
  let re_newline = Regex::new("(\r?\n)").unwrap();
  let re_clean_tags = Regex::new("(</?[ib]>)").unwrap();
  let mut translated_chunks = String::new();
  let mut current_chunk = String::new();
  let mut current_chunk_size = 0;
  const MAX_CHUNK_SIZE: usize = 4000;

  for sub in subs.iter_mut() {
    let mut need_translation = false;

    sub.text = re_clean_tags.replace_all(sub.text.as_str(), "").into();
    sub.text = re_newline.replace_all(sub.text.as_str(), " ").into();

    let colored_text = re_color.replace_all(sub.text.as_str(), |caps: &Captures| {
      let captured_word = caps.get(0).unwrap().as_str();

      if let Some(word) = words.get(captured_word.to_ascii_lowercase().as_str()) {
        if let WordKind::Known = word.kind {} else {
          need_translation = true;

          return format!("<font color=\"#FFFF80\">{}</font>", captured_word);
        }
      }

      String::from(captured_word)
    }).into();

    if need_translation {
      sub.need_translation = true;
      let text: String = re_newline.replace_all(sub.text.as_str(), "*").into();
      let len = text.len();
      current_chunk_size = current_chunk_size + len;

      if current_chunk_size > MAX_CHUNK_SIZE {
        //println!("Original chunk:\n {}\n", current_chunk);
        current_chunk_size = len;
        let translated_chunk = Google {}.translate(current_chunk, Langage::EN, Langage::RU).unwrap();
        sleep(Duration::from_secs(1));
        //println!("Translated chunk:\n {}\n", translated_chunk);
        translated_chunks.push_str(translated_chunk.as_str());
        translated_chunks.push_str("\r\n");
        current_chunk = String::new();
      }

      current_chunk.push_str(text.as_str());
      current_chunk.push_str("\r\n");
      sub.text = colored_text
    }
  }

  if !current_chunk.is_empty() {
    //println!("Original chunk:\n{}\n", current_chunk);
    let translated_chunk = Google {}.translate(current_chunk, Langage::EN, Langage::RU).unwrap();
    //println!("Translated chunk:\n {}\n", translated_chunk);
    translated_chunks.push_str(translated_chunk.as_str());
    translated_chunks.push_str("\r\n");
  }

  translated_chunks = translated_chunks.replace("\\r\\n", "\r\n");
  let mut translated_lines = translated_chunks.lines();

  for sub in subs.iter_mut() {
    if sub.need_translation {
      let translated_text = translated_lines.next().unwrap().replace(" *", "\r\n");
      sub.text.push_str("\r\n");
      sub.text.push_str(translated_text.as_str());
    }
  }
}

fn main() {
  let start = Utc::now();
  let args = get_args();

  if args.analyze_mode {
    println!("Analysis mode");
  }

  println!("Read subs from: '{}'", &args.input_subs_filename);
  let subs_text = load_text_file(&args.input_subs_filename).unwrap();
  let mut subs = parse_subs(&subs_text);

  println!("Read words database from: '{}'", &args.database_filename);
  let db_words_text = load_text_file(&args.database_filename).unwrap_or_default();
  let mut db_words = parse_db_words(&db_words_text);
  println!("{} words is in the database", db_words.len());

  let lowercase_subs_text = subs_text.to_ascii_lowercase();
  let sub_words = parse_sub_words(&lowercase_subs_text);
  println!("Found {} unique words in subs", sub_words.len());
  let words_db_len = db_words.len();

  for (k, v) in sub_words.into_iter() {
    db_words.entry(k).or_insert(v);
  }

  if db_words.len() > words_db_len {
    println!("Add {} new words to the database", db_words.len() - words_db_len);
  } else {
    println!("No new words found");
  }

  let mut sorted_words: Vec<&Word> = db_words.iter().map(|(_, word)| word).collect();
  sorted_words.sort_by(|&left, &right| left.text.cmp(&right.text));

  let mut words_db_text = sorted_words.iter().fold(String::new(), |s, &w| {
    match w.kind {
      WordKind::New => s + "?:" + w.text + "\r\n",
      _ => s,
    }
  });

  words_db_text = sorted_words.iter().fold(words_db_text, |s, &w| {
    match w.kind {
      WordKind::Unknown => s + "u:" + w.text + "\r\n",
      _ => s,
    }
  });

  words_db_text = sorted_words.iter().fold(words_db_text, |s, &w| {
    match w.kind {
      WordKind::Known => s + "k:" + w.text + "\r\n",
      _ => s,
    }
  });

  File::create(&args.database_filename)
    .expect("Failed to open database file for writing")
    .write(words_db_text.as_bytes())
    .expect("Failed to write to the database file");

  if !args.analyze_mode {
    println!("Translate subs");
    translate_subs(&mut subs, &db_words);
    let translated_subs_text = subs.iter().fold(String::new(), |acc, sub| acc + &sub.stringify());

    println!("Write translated subs to: '{}'", &args.output_subs_filename);

    let mut output_file = File::create(&args.output_subs_filename)
      .expect("Failed to open file for writing");

    output_file.write(translated_subs_text.as_bytes())
      .expect("Failed to write to the file");
  }


  let dur = Utc::now().signed_duration_since(start).num_milliseconds();
  println!("Succeed in {} ms", dur);
}
