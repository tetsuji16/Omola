//! OpenModelica error output -> clickable source locations.

/// One `[file:line:column]` location of an OMC message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file_name: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Pick the `[/path/File.mo:12:3-12:20:writable] Error: ...` lines out of OMC output.
///
/// OMC prints the path of the private copy handed to it, so only the file name is kept;
/// the gutter then jumps to the line in whatever buffer is open.
pub fn parse_omc_diagnostics(output: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in output.lines() {
        let Some(open) = line.find('[') else {
            continue;
        };
        let rest = &line[open + 1..];
        let Some(close) = rest.find(']') else {
            continue;
        };
        let inside = &rest[..close];
        let Some(mo) = inside.rfind(".mo:") else {
            continue;
        };
        let path = &inside[..mo + 3];
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned();
        let location = &inside[mo + 4..];
        let mut fields = location.split(':');
        let Some(line_no) = fields
            .next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let Some(column) = fields
            .next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let message = line[open + 1 + close + 1..].trim().to_owned();
        out.push(Diagnostic {
            file_name,
            line: line_no,
            column,
            message,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shaped like OMC 1.27 output; the GUI passes `<Class>.mo` copies to OMC.
    const OUTPUT: &str = "[/tmp/omola-gui/session-1/run-2/BouncingBall.mo:7:3-7:20:writable] Error: Class der not found in scope.\n\
[/home/user/My Models/Bad.mo:12:1-12:4:writable] Warning: Variable x has no binding.\n\
Error: Something without a location.\n";

    #[test]
    fn parses_line_and_column_from_omc_messages() {
        let diagnostics = parse_omc_diagnostics(OUTPUT);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].file_name, "BouncingBall.mo");
        assert_eq!(diagnostics[0].line, 7);
        assert_eq!(diagnostics[0].column, 3);
        assert!(diagnostics[0].message.contains("Class der not found"));
        assert_eq!(diagnostics[1].file_name, "Bad.mo");
        assert_eq!(diagnostics[1].line, 12);
        assert_eq!(diagnostics[1].column, 1);
    }

    #[test]
    fn ignores_messages_without_a_location() {
        assert!(parse_omc_diagnostics("Error: simulate failed\n").is_empty());
        assert!(parse_omc_diagnostics("").is_empty());
    }

    #[test]
    fn malformed_locations_are_skipped() {
        assert!(parse_omc_diagnostics("[/tmp/X.mo:not:numbers] Error: x").is_empty());
    }
}
