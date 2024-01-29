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

    fn next_token(&mut self) -> Option<String> {
        todo!()
    }
}

fn main() -> io::Result<()> {
    let file_path = "docs.gl/gl4/glEnable.xhtml";

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

    let lexer = Lexer::new(&content);

    println!("{content:?}", content = &lexer.content);

    Ok(())
}
