use std::collections::hash_map::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Utc, NaiveTime};
use clap::{App, Arg};
use regex::Regex;
use regex::Captures;
use translate_core::*;
use std::fmt;

struct Args {
  input_subs_filename: String,
  output_subs_filename: Option<String>,
  known_words_filename: Option<String>,
  new_words_filename: Option<String>,
}

struct WordStat<'a> {
  word: &'a str,
  freq: u32,
}

struct Sub {
  index: u32,
  start_time: NaiveTime,
  end_time: NaiveTime,
  text: String,
  need_translation: bool,
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
    .arg(Arg::with_name("input subs")
      .required(true)
      .value_name("INPUT SUBS")
      .help("Sets an input subtitles file")
      .index(1))
    .arg(Arg::with_name("output subs")
      .short("o")
      .long("output-subs")
      .value_name("OUTPUT SUBS")
      .takes_value(true)
      .help("Sets the output subtitles file")
      .index(2))
    .arg(Arg::with_name("known words")
      .short("k")
      .long("known-words")
      .value_name("INPUT KNOWN WORDS")
      .takes_value(true)
      .help("Sets the known words file"))
    .arg(Arg::with_name("new words")
      .short("n")
      .long("new-words")
      .value_name("OUTPUT NEW WORDS")
      .takes_value(true)
      .help("Sets the output new words file"))
    .get_matches();

  let input_subs_filename = matches.value_of("input subs").unwrap().to_owned();
  let mut input_file_path;

  let output_subs_filename = match matches.value_of("output subs") {
    Some(name) => Option::from(name.to_owned()),
    None => {
      input_file_path = PathBuf::from(&input_subs_filename);
      input_file_path.set_extension("output.srt");
      Option::from(input_file_path.to_str().unwrap().to_owned())
    }
  };

  let known_words_filename = match matches.value_of("known words") {
    Some(name) => Option::from(name.to_owned()),
    None => None
  };

  let new_words_filename = match matches.value_of("new words") {
    Some(name) => Option::from(name.to_owned()),
    None => None
  };

  Args {
    input_subs_filename,
    output_subs_filename,
    known_words_filename,
    new_words_filename,
  }
}

fn load_file<P>(file_name: P) -> String where P: AsRef<Path> {
  let mut text = String::new();

  let mut input_file = File::open(file_name)
    .expect("Unable to open file");

  input_file.read_to_string(&mut text)
    .expect("Unable to read file");

  text
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

fn get_word_stats(lowercase_text: &String) -> Vec<WordStat> {
  let mut start_index = std::usize::MAX;
  let mut word_stat_index: HashMap<&str, usize> = HashMap::new();
  let mut word_stats: Vec<WordStat> = Vec::new();

  for (i, c) in lowercase_text.chars().enumerate() {
    if c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '\'' {
      if start_index == std::usize::MAX {
        start_index = i;
      }
    } else if start_index != std::usize::MAX {
      let word = &lowercase_text[start_index..i];

      match word_stat_index.get(word) {
        Some(ind) => {
          let mut word_stat = word_stats.get_mut(*ind).unwrap();
          word_stat.freq = word_stat.freq + 1;
        }
        None => {
          word_stat_index.insert(word, word_stats.len());
          word_stats.push(WordStat { word, freq: 1 });
        }
      }

      start_index = std::usize::MAX;
    }
  }

  word_stats
}

fn translate_subs(subs: &mut Vec<Sub>, known_words: &Vec<&str>) {
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

      // TODO: optimize by case insensitive comparison
      if !known_words.iter().any(|known_word| *known_word == captured_word.to_ascii_lowercase()) {
        need_translation = true;

        return format!("<font color=\"#FFFF80\">{}</font>", captured_word);
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

  println!("Read subs from: '{}'", args.input_subs_filename);
  let subs_text = load_file(args.input_subs_filename);
  let mut subs = parse_subs(&subs_text);

  let known_words_text = if let Some(file_name) = &args.known_words_filename {
    println!("Read known words from: '{}'", file_name);
    let known_words_text = load_file(file_name);
    known_words_text
  } else {
    String::new()
  };

  let lowercase_subs_text = subs_text.to_ascii_lowercase();
  let mut word_stats = get_word_stats(&lowercase_subs_text);
  word_stats.sort_by(|left, right| right.freq.cmp(&left.freq));
  let mut words: Vec<_> = word_stats.iter().map(|word_stat| word_stat.word).collect();
  println!("Found {} unique words", words.len());
  let known_words: Vec<_> = known_words_text.lines().collect();
  println!("Filter out {} known words", known_words.len());
  words.retain(|word| !known_words.contains(word));
  println!("After filter {} unknown words left", words.len());

  if let Some(file_name) = &args.new_words_filename {
    println!("Write new words to: '{}'", file_name);

    let mut output_file = File::create(file_name)
      .expect("Failed to open file for writing");

    for word in &words {
      if !known_words.contains(word) {
        let str = format!("{}\n", word);
        output_file.write(str.as_bytes())
          .expect("Failed to write to the file");
      }
    }
  }

  if let Some(file_name) = &args.output_subs_filename {
    println!("Translate subs");
    translate_subs(&mut subs, &known_words);
    let translated_subs_text = subs.iter().fold(String::new(), |acc, sub| acc + &sub.stringify());

    println!("Write translated subs to: '{}'", file_name);

    let mut output_file = File::create(file_name)
      .expect("Failed to open file for writing");

    output_file.write(translated_subs_text.as_bytes())
      .expect("Failed to write to the file");
  }


  let dur = Utc::now().signed_duration_since(start).num_milliseconds();
  println!("Succeed in {} ms", dur);
}
