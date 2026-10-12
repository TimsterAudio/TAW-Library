//! Scans folders into a library database without the panel, treating each
//! folder as a drive of its own. Handy for testing:
//!
//!     cargo run --release -p mb-core --example scan -- library.db tools/fixtures/Data "tools/fixtures/OLD IPOD"

use mb_core::db::Db;
use mb_core::excludes::Excludes;
use mb_core::scan::{self, Progress};
use mb_core::volumes;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [db_path, folders @ ..] = args.as_slice() else {
        eprintln!("usage: scan <library.db> <folder>...");
        std::process::exit(2);
    };
    let db = Db::open(db_path.as_ref()).expect("open the library");
    let now = mb_core::now_secs();
    for f in folders {
        let path = std::fs::canonicalize(f).expect("folder");
        let v = volumes::folder_volume(&path);
        db.add_location(&v, "", now).expect("add folder");
    }
    let ex = Excludes::new(db.rules().unwrap());
    let jobs = scan::plan(&db, &[]).unwrap();
    let p = Progress::default();
    let t = std::time::Instant::now();
    let s = scan::run(&db, &jobs, &ex, &p, now).unwrap();
    println!(
        "{} music files, {} read, {} gone, {} unreadable, in {:.2} s",
        s.found,
        s.read,
        s.removed,
        s.errors,
        t.elapsed().as_secs_f32()
    );
}
