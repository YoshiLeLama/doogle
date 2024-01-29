use std::fs::File;
use std::io::{self, BufReader};

use xml::reader::XmlEvent;
use xml::EventReader;

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
    let file_path = "docs.gl/gl4/glEnable.xhtml";

    let file = File::open(file_path)?;
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

    while let Some(token) = lexer.next_token() {
        println!("token => {token}");
    }


    Ok(())
}
