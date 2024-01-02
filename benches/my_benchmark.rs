use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};


pub fn get_all_folders_at<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_dir())
        .collect())
}

pub fn get_all_folders_at_iter<P>(path: P) -> std::io::Result<impl Iterator<Item = PathBuf>>
where
    P: AsRef<Path>,
{
    Ok(fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|pathbuf| pathbuf.is_dir())
    )
}




fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("query folders", |b| b.iter(|| {
        let query: &str = "sprites";
        let mut queue: VecDeque<PathBuf> = black_box(VecDeque::from([
            PathBuf::from("C:/Users/ddxte/Documents/"),
        ]));
        let mut results: Vec<PathBuf> = Vec::new();

        loop {
            let Some(search_path) = queue.pop_front() else {
                break;
            };

            let Ok(folders) = get_all_folders_at_iter(search_path) else {
                continue;
            };

            for p in folders {
                if p.display().to_string().to_lowercase() .contains(&query) {
                    results.push(p.clone());
                }
                
                if !p.file_name().and_then(|o| o.to_str()).unwrap_or_default() .starts_with('.') {
                    queue.push_back(p);
                }
            }
        }

        // println!("got {:?}", &results);
        results
   }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
