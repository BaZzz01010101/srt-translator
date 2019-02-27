use std::collections::hash_map::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use chrono::Utc;
use clap::{App, Arg};
use regex::Regex;

struct Args {
  input_file_name: String,
  output_file_name: String,
  blacklist_file_name: Option<String>,
}

fn main() {
  let start = Utc::now();
  let args = process_args();
  process_files(args);
  let dur = Utc::now().signed_duration_since(start).num_milliseconds();

  println!("Succeed in {} ms", dur);
}

fn process_args() -> Args {
  let matches = App::new("Word Parser")
    .version("1.0")
    .author("ZeuS <andy2002ua@gmail.com>")
    .about("Parse given text file and create a list of words filtered by black list and sorted by hit rate")
    .arg(Arg::with_name("input")
      .required(true)
      .value_name("INPUT FILE")
      .help("Sets an input file to use")
      .index(1))
    .arg(Arg::with_name("output")
      .short("o")
      .long("out")
      .value_name("OUTPUT FILE")
      .help("Sets the output file to use")
      .index(2))
    .arg(Arg::with_name("blacklist")
      .short("b")
      .long("blacklist")
      .value_name("BLACKLIST FILE")
      .takes_value(true)
      .help("Sets the blacklist file to use"))
    .get_matches();

  let input_file_name = matches.value_of("input").unwrap().to_owned();
  let mut input_file_path;

  let output_file_name = match matches.value_of("output") {
    Some(name) => name.to_owned(),
    None => {
      input_file_path = PathBuf::from(&input_file_name);
      input_file_path.set_extension("output.txt");
      input_file_path.to_str().unwrap().to_owned()
    }
  };

  let blacklist_file_name = match matches.value_of("blacklist") {
    Some(name) => Option::from(name.to_owned()),
    None => None
  };

  Args {
    input_file_name,
    output_file_name,
    blacklist_file_name,
  }
}

fn process_files(args: Args) {
  let mut input_file = File::open(args.input_file_name)
    .expect("Unable to open input file");

  let mut input_string = String::new();
  input_file.read_to_string(&mut input_string).expect("Unable to read input File");

  //let input_words = extract_words(&input_string);
  //let folded_input_words = fold_words(input_words);

  input_string.make_ascii_lowercase();
  let word_stats = build_word_stats(&input_string);
  let sentences = collect_sentences(&input_string);
  println!("Matched {} sentences", sentences.len());

  if let Some(file_name) = args.blacklist_file_name {
    File::open(file_name).expect("Blacklist file not found");
  }

  let mut output_file = File::create(args.output_file_name).unwrap();

  for word_stat in &word_stats {
    let str = format!("{}: {}\n", word_stat.freq, word_stat.word);
    output_file.write(str.as_bytes());
  }
}

fn extract_words(text: &String) -> Vec<&str> {
  let mut start_index = std::usize::MAX;
  let mut words = Vec::new();

  for (i, c) in text.chars().enumerate() {
    if c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '\'' {
      if start_index == std::usize::MAX {
        start_index = i;
      }
    } else {
      if start_index != std::usize::MAX {
        let word = &text[start_index..i];
        //println!("word found: {}", word);
        words.push(word);
        start_index = std::usize::MAX;
      }
    }
  }

  words
}

struct WordStat<'a> {
  word: &'a str,
  freq: u32,
}

fn fold_words<'a>(input_words: Vec<&'a str>) -> Vec<&'a WordStat> {
  let mut word_stats: HashMap<&str, WordStat> = HashMap::new();

  for (i, word) in input_words.iter().enumerate() {
    match word_stats.get_mut(word) {
      Some(word_stat) => {
        word_stat.freq = word_stat.freq + 1;
      }
      None => {
        word_stats.insert(word, WordStat { word, freq: 1 });
      }
    }
  }

  for (i, word_stat) in word_stats.values().enumerate() {
    println!("{} : {}", word_stat.word, word_stat.freq);
  }

  let word_stat = Vec::new();

  word_stat
}

fn build_word_stats(text: &String) -> Vec<WordStat> {
  let mut start_index = std::usize::MAX;
  let mut word_stat_index: HashMap<&str, usize> = HashMap::new();
  let mut sorted_word_stats: Vec<WordStat> = Vec::new();

  for (i, c) in text.chars().enumerate() {
    if c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '\'' {
      if start_index == std::usize::MAX {
        start_index = i;
      }
    } else if start_index != std::usize::MAX {
      let word = &text[start_index..i];

      match word_stat_index.get(word) {
        Some(ind) => {
          let mut word_stat = sorted_word_stats.get_mut(*ind).unwrap();
          word_stat.freq = word_stat.freq + 1;
        }
        None => {
          //let mut word_stat = WordStat { word: word, freq: 1 };
          //word_stats.insert(word, &mut word_stat);
          word_stat_index.insert(word, sorted_word_stats.len());
          sorted_word_stats.push(WordStat { word, freq: 1 });
        }
      }

      //println!("word found: {}", word);
      //words.push(word);
      start_index = std::usize::MAX;
    }
  }

  sorted_word_stats.sort_by(|left, right| right.freq.cmp(&left.freq));

  sorted_word_stats
}

struct Sentence<'a> {
  orig: &'a str,
  begin: usize,
  end: usize,
}

fn collect_sentences(text: &String) -> Vec<Sentence> {
  let mut sentences = Vec::new();
  //let re = Regex::new(r"(?m)^\d+.?$^\d{2}:\d{2}:\d{2},\d{3} --> \d{2}:\d{2}:\d{2},\d{3}.?$((?:^(?:<i>)?.+(?:<i>)?.?$)+)^.?$").unwrap();

  let re = Regex::new(r"(?ms)\d+\r?\n\d{2}:\d{2}:\d{2},\d{3} --> \d{2}:\d{2}:\d{2},\d{3}\r?\n(.+?)\r?\n\r?\n").unwrap();
  //let re = Regex::new(r"(?ms)(.+?)(?:\r\n\r\n)").unwrap();

  //println!("is-match: {}", re.is_match(text));
  for caps in re.captures_iter(text) {
    let m = caps.get(1).unwrap();

    sentences.push(Sentence {
      orig: m.as_str(),
      begin: m.start(),
      end: m.end(),
    });
  }

  sentences
}