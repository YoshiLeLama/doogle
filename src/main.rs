use std::collections::{HashMap, HashSet};
use std::fs::{File, self};
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process::exit;

use xml::reader::XmlEvent;
use xml::EventReader;

type TermCount = HashMap<String, usize>;

type TermFreq = HashMap<String, f64>;
type TermFreqIndex = HashMap<PathBuf, TermFreq>;
type InvDocFreq = HashMap<String, f64>;

struct Lexer<'a> {
    content: &'a [char]
}

impl<'a> Lexer<'a> {
    fn new(content: &'a [char]) -> Self {
        Self {content}
    }

    fn trim_left(&mut self) {
        while !self.content.is_empty() && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }
    }

    fn chop(&mut self, n: usize) -> &'a [char] {
        let token = &self.content[..n];
        self.content = &self.content[n..];
        token
    }

    fn chop_while<P>(&mut self, mut predicate: P) -> &'a [char] 
        where P: FnMut(&char) -> bool
    {
        let mut n = 0;
        while n < self.content.len() && predicate(&self.content[n]) {
            n += 1;
        }
        
        self.chop(n)
    }

    fn next_token(&mut self) -> Option<String> {
        self.trim_left();

        if self.content.is_empty() {
            return None;
        }

        if self.content[0].is_numeric() {
            return Some(self.chop_while(|x| x.is_numeric()).iter().collect());
        }

        if self.content[0].is_alphabetic() {
            return Some(self.chop_while(|x| x.is_alphanumeric()).iter().collect());
        }

        Some(self.chop(1).iter().collect())
    }
}

fn main() -> io::Result<()> {
    let dir_path = "docs.gl/gl4";
    let dir = fs::read_dir(&dir_path)?;

    let mut tfi = TermFreqIndex::new();

    for file in dir {
        let file = file?;
        let file_path = file.path();

        let file = File::open(&file_path)?;
        let file = BufReader::new(file);

        let mut content = String::new(); 

        let parser = EventReader::new(file);
        for e in parser {
            match e {
                Ok(XmlEvent::Characters(chars)) => {
                    content.push_str(&chars);
                    content.push(' ');
                }
                Err(e) => {
                    println!("ERROR while parsing XML : {e}");
                }
                _ => {}
            }
        }

        let content = content.chars().collect::<Vec<_>>();

        let mut lexer = Lexer::new(&content);
        let mut terms_count = 0;
        let mut tc = TermCount::new();

        while let Some(token) = lexer.next_token() {
            let term = token.to_uppercase();
            if let Some(count) = tc.get_mut(&term) {
                *count += 1;
            } else {
                tc.insert(term, 1);
            }
            terms_count += 1;
        }

        let mut tf = TermFreq::new();

        assert!(terms_count != 0);

        for (term, count) in tc {
            tf.insert(term.to_string(), count as f64 / terms_count as f64);
        }

        tfi.insert(file_path, tf);
    }

    let mut idf = InvDocFreq::new();

    let docs_count = tfi.len();

    for tf in tfi.values() {
        for term in tf.keys() {
            if !idf.contains_key(term) {
                let mut term_appearences = 0;
                for tf in tfi.values() {
                    if tf.contains_key(term) {
                        term_appearences += 1;
                    }
                }

                idf.insert(term.to_string(), (docs_count as f64 / term_appearences as f64).log2());
            }
        }
    }

    let mut results = Vec::<(PathBuf, f64)>::new();

    let mut request = String::new();
    std::io::stdin().read_line(&mut request).unwrap();
    let request = request.trim_end();

    println!("Results for {request}");

    let request = request.to_uppercase();
    let idf_test = match idf.get(&request) {
        Some(idf_value) => idf_value,
        None => &0.,
    };

    for (path,tf) in tfi {
        let tf_test = match tf.get(&request) {
            Some(tf_value) => tf_value,
            None => &0.,
        };
        let tfidf = tf_test * idf_test;
        results.push((path, tfidf));
    }

    results.sort_by(|(_,v1), (_,v2)| v1.partial_cmp(v2).unwrap());
    results.reverse();

    for (path, tfidf) in results.iter().take(10) {
        if tfidf == &0. {
            break;
        }
         
        println!("{:?} {}", path, tfidf);
    }

    Ok(())
}
