use vt100::Parser;

fn main() {
    let mut parser = Parser::new(24, 80, 100); // 100 lines of scrollback
    
    // Simulate some output
    for i in 0..50 {
        parser.process(format!("Line {}\n", i).as_bytes());
    }
    
    let screen = parser.screen();
    println!("Scrollback len: {}", screen.scrollback());
    
    // Try to access scrollback?
    // screen.rows() only gives visible 24 rows?
    let visible_rows = screen.rows(0, 80).count();
    println!("Visible rows: {}", visible_rows);
    
    // Check if we can get previous lines
    // vt100 0.15 doesn't seem to expose 'get_row(index)'
}

