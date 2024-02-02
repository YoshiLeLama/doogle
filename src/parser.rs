use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;

use xml::reader::XmlEvent;
use xml::EventReader;

pub fn parse_xml_file(path: &PathBuf) -> Result<Vec<char>, ()> {
    let file = match File::open(path) {
        Ok(file) => BufReader::new(file),
        Err(e) => { eprintln!("{e}"); return Err(()); }
    };

    let mut content = String::new(); 

    let parser = EventReader::new(BufReader::new(file));
    for e in parser {
        match e {
            Ok(XmlEvent::Characters(chars)) => {
                content.push_str(&chars);
                content.push(' ');
            }
            Err(e) => {
                eprintln!("ERROR parsing XML : {e}");
                return Err(());
            }
            _ => {}
        }
    }

    Ok(content.chars().collect::<Vec<_>>())
}
