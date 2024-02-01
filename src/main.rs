use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, Write, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Instant};

use xml::reader::XmlEvent;
use xml::EventReader;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Document {
    terms_count: usize,
    last_modified: SystemTime,
    tf: TermFreq,
}

type TermFreq = HashMap<String, usize>;
type TermFreqIndex = HashMap<PathBuf, Document>;
type InvDocFreq = HashMap<String, usize>;

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

impl<'a> Iterator for Lexer<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

fn parse_xml_file(path: &PathBuf) -> Result<Vec<char>, ()> {
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

#[derive(Deserialize, Serialize)]
struct Model {
    dirs: HashSet<PathBuf>,
    tfi: TermFreqIndex,
    idf: InvDocFreq,
    num_docs: usize,
}

impl Model {
    fn new() -> Self {
        Self {
            dirs: HashSet::new(),
            tfi: TermFreqIndex::new(),
            idf: InvDocFreq::new(),
            num_docs: 0,
        }
    }

    fn save_to_file(&self, file_name: &str) {
        println!("Saving model to {file_name}...");
        let file = File::create(file_name).unwrap();
        serde_json::to_writer(BufWriter::new(file), self).unwrap();
        println!("Done saving.")
    }

    fn load_from_file(file_name: &str) -> Self {
        println!("Loading the model from {file_name}...");
        let file = File::open(file_name).unwrap();
        let mut model: Self = serde_json::from_reader(BufReader::new(file)).unwrap();

        let mut invalid_paths: Vec<PathBuf> = Vec::new();
        let mut updated_paths: Vec<(SystemTime, PathBuf)> = Vec::new();
        
        for (file_path, doc) in &model.tfi {
            match fs::metadata(&file_path) {
                Ok(metadata) => {
                    let last_modified = metadata.modified().unwrap(); 
                    if last_modified != doc.last_modified {
                        updated_paths.push((last_modified, file_path.to_path_buf())); 
                    }
                }
                Err(_) => { invalid_paths.push(file_path.to_path_buf()) }
            }
        }

        for doc_path in invalid_paths {
            println!("Invalidating {doc_path:?}...");
            model.remove_doc(&doc_path);
        }

        'update_iter: for (last_modified, doc_path) in updated_paths {
            println!("Updating {doc_path:?}...");
            let content = match parse_xml_file(&doc_path) {
                Ok(tokens) => tokens,
                Err(_) => { eprintln!("ERROR while parsing xml document"); continue 'update_iter; }
            };
            model.add_doc(doc_path, &content, last_modified);
        }

        println!("Done loading model.");

        model
    }

    fn add_dir(&mut self, dir_path: &PathBuf) -> Result<(), ()> {
        println!("Indexing directory : {dir_path:?}");
        let dir = fs::read_dir(dir_path).map_err(|err| eprintln!("{err}"))?;

        self.dirs.insert(dir_path.to_path_buf());

        'files_iter: for file in dir {
            let file = file.map_err(|err| eprintln!("ERROR file is incorrect : {err}"))?;
            let file_path = file.path();
            let file_type = file.file_type().map_err(|err| eprintln!("ERROR when querying file type : {err}"))?;

            if file_type.is_dir() {
                self.add_dir(&file_path)?;
                continue 'files_iter;
            }

            let file_ext = file_path.extension().and_then(std::ffi::OsStr::to_str);
            let last_modified = file.metadata().map_err(|err| eprintln!("ERROR when querying metadata : {err}"))?
                .modified().map_err(|err| eprintln!("ERROR when querying last modified time : {err}"))?;

            println!("  modified on {last_modified:?}");

            if let Some(ext) = file_ext {
                let content = match ext {
                    "xhtml" => parse_xml_file(&file_path)?,
                    _ => { println!("Skipping file {file_path:?}"); continue 'files_iter; } // Skipping all files that we cannot parse yet
                };
                self.add_doc(file_path, &content, last_modified);
            } else {
                println!("Unknown file : {file_path:?}");
            }
        }

        Ok(())
    }

    fn add_doc(&mut self, doc_path: PathBuf, content: &[char], last_modified: SystemTime) {
        self.remove_doc(&doc_path);

        println!("Indexing document : {doc_path:?}");

        let mut lexer = Lexer::new(content);
        let mut terms_count = 0;
        let mut tf = TermFreq::new();

        while let Some(token) = lexer.next_token() {
            let term = Model::clean_term(&token);

            // Add to tf
            if let Some(count) = tf.get_mut(&term) {
                *count += 1;
            } else {
                tf.insert(term.clone(), 1);
            }

            terms_count += 1;
        }

        // Add to idf
        for term in tf.keys() {
            if let Some(count) = self.idf.get_mut(term) {
                *count += 1;
            } else {
                self.idf.insert(term.clone(), 1);
            }
        }

        self.tfi.insert(doc_path, Document { terms_count , last_modified , tf });
        self.num_docs += 1;
    }

    fn remove_doc(&mut self, doc_path: &PathBuf) {
        if let Some(doc) = self.tfi.remove(doc_path) {
            self.num_docs -= 1;

            for term in doc.tf.keys() {
                if let Some(count) = self.idf.get_mut(term) {
                    *count -= 1;
                } 
            }
        }
    }

    fn clean_term(term: &str) -> String {
        term.to_uppercase()
    }

    fn get_idf(&self, term: &str) -> f32 {
        match self.idf.get(term) {
            Some(&v) if v > 0 => {
                let n = self.num_docs as f32;
                let v = v as f32;
                (n / v).log10()
            },
            _ => 0.
        }
    }

    fn get_tf_doc(&self, doc_path: &PathBuf, term: &str) -> f32 {
        match self.tfi.get(doc_path) {
            Some(doc) if doc.terms_count > 0 => match doc.tf.get(term) {
                Some(&v) => {
                    let terms_count = doc.terms_count as f32;
                    let v = v as f32;
                    v / terms_count
                },
                None => 0.
            }
            _ => 0.
        }
    }

    fn process_query(&self, request: &str) -> HashMap<PathBuf, f32> {
        let request = request.split_whitespace().collect::<Vec<_>>();

        let mut results = HashMap::<PathBuf, f32>::new();

        'idf_iter: for term in request {
            let term = Model::clean_term(term);
           
            let idf_value = self.get_idf(&term);
            if idf_value == 0. {
                continue 'idf_iter; // Skipping all calculations if term isn't in the corpus
            }

            'tf_iter: for path in self.tfi.keys() {
                let tf_value = self.get_tf_doc(path, &term);
                if tf_value == 0. {
                    // Skip to the next document if term isn't in the current one
                    continue 'tf_iter; 
                }

                let tfidf_value = tf_value * idf_value;

                match results.get_mut(path) {
                    Some(v) => { *v += tfidf_value; }
                    None => { results.insert(path.to_path_buf(), tfidf_value); }
                }
            }
        }

        results
    }
}

fn prompt_request() -> Result<String, ()> {
    let mut request = String::new();
    print!("> ");
    std::io::stdout().flush().map_err(|err| eprintln!("ERROR when flushing stdout : {err}"))?;
    std::io::stdin().read_line(&mut request).unwrap();
    Ok(request.trim_end().to_string())
}

fn main() -> Result<(), ()> {
    let save_file_name = "index.json";
    let mut model;

    let loading_start = Instant::now();
    if Path::new(save_file_name).exists() {
        model = Model::load_from_file(save_file_name);
        println!("Took {elapsed:.2}s to load the model!", elapsed = loading_start.elapsed().as_secs_f32());
    } else {
        println!("Creating the model...");
        model = Model::new();
        model.add_dir(&PathBuf::from("docs.gl"))?;
        println!("Took {elapsed:.2}s to create the model!", elapsed = loading_start.elapsed().as_secs_f32());
    }

    println!("Search among {length} files!", length = model.tfi.len());

    'request_loop: loop {
        let res_compute_start = Instant::now();

        let request = match prompt_request() {
            Ok(v) if v != ":quit" => v,
            _ => break 'request_loop,
        };

        let results = model.process_query(&request);
        let mut results = results.iter().collect::<Vec<_>>();
        results.sort_by(|(_,v1),(_,v2)| v2.partial_cmp(v1).unwrap());
        
        if results.is_empty() {
            println!("No result for {request}");
        } else {
            println!("Results for {request} (retrieved in {elapsed:.2}s)", elapsed = res_compute_start.elapsed().as_secs_f32());

            'result_display: for (path, &tfidf) in results.iter().take(20) {
                if tfidf == 0. {
                    break 'result_display;
                }
                 
                println!("{:?} {}", path, tfidf);
            }
        }
    }

    model.save_to_file(save_file_name);

    Ok(())
}
