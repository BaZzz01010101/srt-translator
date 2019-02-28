# srt-translator
CLI tool for selective translation of SRT subtitles

The idea is to:
- parse an english SRT file, extract all english words
- filter them using blacklist file which contains all known words
- translate all sentences with unknown words using Google Translate REST API
- insert thanslation next to the original sentence 
