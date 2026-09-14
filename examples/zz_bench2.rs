use std::time::Instant;
fn main() {
    for path in std::env::args().skip(1) {
        let src = std::fs::read_to_string(&path).unwrap();
        let r = compiler_rs_lib::compile(&src, 1);
        let mut c_best = f64::MAX;
        let mut f_best = f64::MAX;
        for _ in 0..10 {
            let t = Instant::now();
            let r2 = compiler_rs_lib::compile(&src, 1);
            c_best = c_best.min(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&r2);
        }
        for _ in 0..10 {
            let mut out = Vec::with_capacity(src.len() * 2);
            let t = Instant::now();
            compiler_rs_lib::fmt::format(
                &mut out,
                r.ast_root,
                &r.ast_nodes,
                &r.comments,
                &r.control_directives,
                &r.attributes,
                &src,
            )
            .unwrap();
            f_best = f_best.min(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&out);
        }
        println!(
            "{:<18} compile {:>7.2} ms   format {:>7.2} ms   ({} nodes)",
            path.rsplit('/').next().unwrap(),
            c_best,
            f_best,
            r.ast_nodes.len()
        );
    }
}
