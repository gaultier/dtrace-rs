use std::time::Instant;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    for path in &args[1..] {
        let src = std::fs::read_to_string(path).unwrap();
        // Warm up.
        let _ = compiler_rs_lib::compile(&src, 1);
        let iters = if src.len() > 500_000 { 3 } else { 10 };
        let mut best = f64::MAX;
        let mut errs = 0;
        for _ in 0..iters {
            let t = Instant::now();
            let r = compiler_rs_lib::compile(&src, 1);
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            errs = r.errors.len();
        }
        println!(
            "{:<22} {:>8.2} ms  ({} KB, {} errors)",
            path.rsplit('/').next().unwrap(),
            best,
            src.len() / 1024,
            errs
        );
    }
}
