mod lexer;
mod model;
mod parser;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
        model = model::Model::load_from_file(save_file_name);
        println!("Took {elapsed:.2}s to load the model!", elapsed = loading_start.elapsed().as_secs_f32());
    } else {
        println!("Creating the model...");
        model = model::Model::new();
        model.add_dir(&PathBuf::from("docs.gl"))?;
        println!("Took {elapsed:.2}s to create the model!", elapsed = loading_start.elapsed().as_secs_f32());
    }

    println!("Search among {length} files!", length = model.corpus_size());

    println!("(type :quit when you're done)");

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
