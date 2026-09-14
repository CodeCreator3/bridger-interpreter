fn main() {
    // Generates src/parser/grammar.rs into OUT_DIR from src/parser/grammar.lalrpop.
    lalrpop::process_root().expect("lalrpop grammar failed to build");
}
