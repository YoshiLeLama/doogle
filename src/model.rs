use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::lexer;
use crate::parser;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Document {
    terms_count: usize,
    last_modified: SystemTime,
    tf: TermFreq,
}

type TermFreq = HashMap<String, usize>;
type TermFreqIndex = HashMap<PathBuf, Document>;
type InvDocFreq = HashMap<String, usize>;

type QueryResult = HashMap<PathBuf, f32>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Model {
    dirs: HashSet<PathBuf>,
    tfi: TermFreqIndex,
    idf: InvDocFreq,
    num_docs: usize,
}

impl Model {
    pub fn new() -> Self {
        Self {
            dirs: HashSet::new(),
            tfi: TermFreqIndex::new(),
            idf: InvDocFreq::new(),
            num_docs: 0,
        }
    }

    pub fn corpus_size(&self) -> usize {
        self.tfi.len()
    }

    pub fn save_to_file(&self, file_name: &str) {
        println!("Saving model to {file_name}...");
        let file = File::create(file_name).unwrap();
        serde_json::to_writer(BufWriter::new(file), self).unwrap();
        println!("Done saving.")
    }

    pub fn load_from_file(file_name: &str) -> Self {
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
                Err(_) => invalid_paths.push(file_path.to_path_buf()),
            }
        }

        for doc_path in invalid_paths {
            println!("Invalidating {doc_path:?}...");
            model.remove_doc(&doc_path);
        }

        'update_iter: for (last_modified, doc_path) in updated_paths {
            println!("Updating {doc_path:?}...");
            let content = match parser::parse_xml_file(&doc_path) {
                Ok(tokens) => tokens,
                Err(_) => {
                    eprintln!("ERROR while parsing xml document");
                    continue 'update_iter;
                }
            };
            model.add_doc(doc_path, &content, last_modified);
        }

        println!("Done loading model.");

        model
    }

    pub fn add_dir(&mut self, dir_path: &PathBuf) -> Result<(), ()> {
        println!("Indexing directory : {dir_path:?}");
        let dir = fs::read_dir(dir_path).map_err(|err| eprintln!("{err}"))?;

        self.dirs.insert(dir_path.to_path_buf());

        'files_iter: for file in dir {
            let file = file.map_err(|err| eprintln!("ERROR file is incorrect : {err}"))?;
            let file_path = file.path();
            let file_type = file
                .file_type()
                .map_err(|err| eprintln!("ERROR when querying file type : {err}"))?;

            if file_type.is_dir() {
                self.add_dir(&file_path)?;
                continue 'files_iter;
            }

            let file_ext = file_path.extension().and_then(std::ffi::OsStr::to_str);
            let last_modified = file
                .metadata()
                .map_err(|err| eprintln!("ERROR when querying metadata : {err}"))?
                .modified()
                .map_err(|err| eprintln!("ERROR when querying last modified time : {err}"))?;

            if let Some(ext) = file_ext {
                let content = match ext {
                    "xhtml" => parser::parse_xml_file(&file_path)?,
                    _ => {
                        println!("Skipping file {file_path:?}");
                        continue 'files_iter;
                    } // Skipping all files that we cannot parse yet
                };
                self.add_doc(file_path, &content, last_modified);
            } else {
                println!("Unknown file : {file_path:?}");
            }
        }

        Ok(())
    }

    pub fn add_doc(&mut self, doc_path: PathBuf, content: &[char], last_modified: SystemTime) {
        self.remove_doc(&doc_path);

        println!("Indexing document : {doc_path:?}");

        let mut lexer = lexer::Lexer::new(content);
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

        self.tfi.insert(
            doc_path,
            Document {
                terms_count,
                last_modified,
                tf,
            },
        );
        self.num_docs += 1;
    }

    pub fn remove_doc(&mut self, doc_path: &PathBuf) {
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
            }
            _ => 0.,
        }
    }

    fn get_tf_doc(&self, doc_path: &PathBuf, term: &str) -> f32 {
        match self.tfi.get(doc_path) {
            Some(doc) if doc.terms_count > 0 => match doc.tf.get(term) {
                Some(&v) => {
                    let terms_count = doc.terms_count as f32;
                    let v = v as f32;
                    v / terms_count
                }
                None => 0.,
            },
            _ => 0.,
        }
    }

    fn process_query_term(&self, term: &str) -> QueryResult {
        let term = Model::clean_term(term);

        let idf_value = self.get_idf(&term);
        if idf_value == 0. {
            return QueryResult::new();
        }

        let mut results = QueryResult::new();

        for path in self.tfi.keys() {
            let tf_value = self.get_tf_doc(path, &term);
            if tf_value == 0. {
                // Skip to the next document if term isn't in the current one
                continue;
            }

            let tfidf_value = tf_value * idf_value;

            match results.get_mut(path) {
                Some(v) => {
                    *v += tfidf_value;
                }
                None => {
                    results.insert(path.to_path_buf(), tfidf_value);
                }
            }
        }

        results
    }
}

fn combine_results(results: &mut QueryResult, result: QueryResult) {
    for (doc, tfidf) in result {
        match results.get_mut(&doc) {
            Some(curr_tfidf) => {
                *curr_tfidf += tfidf;
            }
            None => {
                results.insert(doc.clone(), tfidf);
            }
        }
    }
}

// Returns a Vec containing (start of task, charge of task)
fn dispatch_tasks(threads_count: usize, tasks: usize) -> Vec<(usize, usize)> {
    // Make sure threads_count is greater than 0 (otherwise no thread would be created)
    // and smaller than the number of terms in the query (otherwise useless threads would be
    // created)
    let threads_count = threads_count.max(1).min(tasks);

    let threads_min_charge = tasks / threads_count;
    let overcharged_threads = tasks % threads_count;

    let mut dispatched = Vec::new();

    for i in 0..threads_count {
        let start = i.min(overcharged_threads) * (threads_min_charge + 1)
            + if i > overcharged_threads {
                i - overcharged_threads
            } else {
                0
            } * threads_min_charge;

        let thread_charge = if i < overcharged_threads {
            threads_min_charge + 1
        } else {
            threads_min_charge
        };

        dispatched.push((start, thread_charge));
    }

    dispatched
}

pub fn process_query(
    model: Arc<Model>,
    request: &str,
    threads_count: usize,
) -> HashMap<PathBuf, f32> {
    let request = request.split_whitespace().collect::<Vec<_>>();

    let mut results = QueryResult::new();
    let mut result_threads = Vec::new();
    let dispatched = dispatch_tasks(threads_count, request.len());

    for (start, thread_charge) in dispatched {
        let model_ref = model.clone();
        let terms = request[start..]
            .iter()
            .take(thread_charge)
            .map(|&s| String::from(s))
            .collect::<Vec<_>>();

        result_threads.push(thread::spawn(move || {
            let mut results = QueryResult::new();
            for term in terms {
                combine_results(&mut results, model_ref.process_query_term(&term));
            }
            results
        }));
    }

    let mut i = request.len();
    while let Some(result_thread) = result_threads.pop() {
        i -= 1;
        match result_thread.join() {
            Ok(result) => {
                combine_results(&mut results, result);
            }
            Err(_) => eprintln!("Something went wrong when joining thread {i}"),
        }
    }

    results
}
