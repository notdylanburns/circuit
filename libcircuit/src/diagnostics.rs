use crate::{
    ansi::{self, AnsiColourType, AnsiModify},
    loader::Loader,
    util::{Pos, Position},
};
use std::fmt::Write;

#[derive(Default, Debug)]
pub struct Context {
    pos: Pos,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DiagnosticKind {
    Info,
    Warning,
    Error,
}

impl DiagnosticKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticKind::Info => "info",
            DiagnosticKind::Warning => "warning",
            DiagnosticKind::Error => "error",
        }
    }

    pub fn colour(&self) -> AnsiColourType {
        match self {
            DiagnosticKind::Info => ansi::BLUE,
            DiagnosticKind::Warning => ansi::YELLOW,
            DiagnosticKind::Error => ansi::RED,
        }
    }
}

impl core::fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

macro_rules! diagnostic {
    (
        $level:ident,
        $d:expr
    ) => {
        Diagnostic::new(
            $d,
            $crate::diagnostics::DiagnosticKind::$level,
            $crate::diagnostics::Context::default()
        )
    };
    (
        $level:ident,
        $d:expr
    ) => {
        diagnostic!($level, $d,)
    };
    (
        $level:ident,
        $d:expr,
        $($ctx:tt)*
    ) => {
        diagnostic!(
            @buildctx diagnostic!($level, $d),
            $($ctx)*
        )
    };
    (@buildctx $d:expr$(,)?) => {
        $d
    };
    (
        @buildctx $d:expr,
        pos = $pos:expr,
        $($ctx:tt)*
    ) => {
        diagnostic!(
            @buildctx $d.with_pos($pos),
            $($ctx)*
        )
    };
    (
        @buildctx $d:expr,
        pos = $pos:expr
    ) => {
        diagnostic!(
            @buildctx $d,
            pos = $pos,
        )
    };
    (
        @buildctx $d:expr,
        note = $note:expr,
        $($ctx:tt)*
    ) => {
        diagnostic!(
            @buildctx $d.add_note($note),
            $($ctx)*
        )
    };
    (
        @buildctx $d:expr,
        note = $note:expr
    ) => {
        diagnostic!(
            @buildctx $d,
            note = $note,
        )
    };
}

pub trait DiagnosticItem: core::fmt::Display + core::fmt::Debug + 'static {}
impl<T> DiagnosticItem for T where T: core::fmt::Display + core::fmt::Debug + 'static {}

pub trait IntoDiagnostic {
    fn into_diagnostic(self, kind: DiagnosticKind, ctx: Context) -> Diagnostic;
}

impl<T> IntoDiagnostic for T
where
    T: DiagnosticItem,
{
    fn into_diagnostic(self, kind: DiagnosticKind, ctx: Context) -> Diagnostic {
        Diagnostic::new(self, kind, ctx)
    }
}

#[derive(Debug)]
pub struct Diagnostic {
    ctx: Context,
    kind: DiagnosticKind,
    diagnostic: Box<dyn DiagnosticItem>,
    notes: Vec<Self>,
}

impl Diagnostic {
    pub fn new<T: DiagnosticItem>(d: T, kind: DiagnosticKind, ctx: Context) -> Self {
        Self {
            ctx,
            kind,
            diagnostic: Box::new(d),
            notes: Vec::new(),
        }
    }

    pub fn kind(&self) -> DiagnosticKind {
        self.kind
    }

    pub fn pos(&self) -> Pos {
        self.ctx.pos
    }

    pub fn set_pos<P: Position>(&mut self, pos: P) -> Pos {
        std::mem::replace(&mut self.ctx.pos, pos.pos())
    }

    pub fn with_pos<P: Position>(mut self, pos: P) -> Self {
        self.set_pos(pos.pos());
        self
    }

    pub fn add_note(mut self, d: Self) -> Self {
        self.notes.push(d);
        self
    }

    pub fn to_string(&self, loader: &mut Loader) -> Result<String, std::fmt::Error> {
        let mut string = String::new();
        let f = &mut string;

        let bold = style!(bold);

        let module_id = self.pos().module_id().expect("no module in pos");

        let module = loader
            .get_module_mut(module_id)
            .unwrap_or_else(|| unreachable!("unknown module in diagnostic: {}", module_id));

        write!(f, "{}{}", bold, module.path_str())?;

        match self.ctx.pos.loc() {
            Some((line, col)) => writeln!(f, ":{}:{} ", line + 1, col + 1)?,
            _ => writeln!(f, "")?,
        };

        write!(f, "{}", bold.inverse())?;

        writeln!(
            f,
            "  {}: {}",
            self.kind
                .as_str()
                .fg(self.kind.colour())
                .apply(style!(bold)),
            self.diagnostic
        )?;

        let Pos::Pos {
            line, col, span, ..
        } = self.pos()
        else {
            return Ok(string);
        };

        let line_no_length = f64::log10(line as f64) as usize + 2;
        let pad = usize::max(line_no_length, 4);
        let barpad = pad + 2;

        let line_style = style!(bold).fg(ansi::BLUE);
        writeln!(f, "{:>barpad$}", "|".apply(line_style))?;

        let line_str = module.read_line(line).expect("failed to read module");

        writeln!(
            f,
            "{:>pad$} {} {}{}{}",
            (line + 1).apply(line_style),
            "|".apply(line_style),
            &line_str[..col],
            &line_str[col..col + span].fg(self.kind.colour()),
            &line_str[col + span..],
        )?;

        writeln!(
            f,
            "{:>barpad$} {:>col$}{}{:~>span$}{}",
            "|".apply(line_style),
            "",
            style!().fg(self.kind.colour()),
            "",
            style!().fg(ansi::RESET),
        )?;

        let note_style = style!(bold).fg(ansi::MAGENTA);

        for note in self.notes.iter() {
            writeln!(
                f,
                "{:>barpad$}",
                "---".apply(note_style),
                barpad = barpad + 1
            )?;
            writeln!(
                f,
                "{:>barpad$} {}: {}",
                "+".apply(note_style),
                note.kind
                    .as_str()
                    .fg(note.kind.colour())
                    .apply(style!(bold)),
                note.diagnostic
            )?;

            let Some(note_module_id) = note.ctx.pos.module_id() else {
                continue;
            };

            let module = loader
                .get_module_mut(note_module_id)
                .unwrap_or_else(|| unreachable!("invalid module in diagnostics: {note_module_id}"));

            if note_module_id != module_id {
                write!(
                    f,
                    "{:>barpad$} == {}",
                    "+".apply(note_style),
                    module.path_str(),
                )?;

                match note.ctx.pos.loc() {
                    Some((line, col)) => writeln!(f, ":{}:{} ==", line + 1, col + 1)?,
                    _ => writeln!(f, "==")?,
                };
            };

            let Pos::Pos {
                line, col, span, ..
            } = note.ctx.pos
            else {
                continue;
            };

            writeln!(f, "{:>barpad$}", "|".apply(line_style))?;

            let line_str = module.read_line(line).expect("failed to read module");

            writeln!(
                f,
                "{:>pad$} {} {}{}{}",
                (line + 1).apply(line_style),
                "|".apply(line_style),
                &line_str[..col],
                &line_str[col..col + span].fg(note.kind.colour()),
                &line_str[col + span..],
            )?;

            writeln!(
                f,
                "{:>barpad$} {:>col$}{}{:~>span$}{}",
                "|".apply(line_style),
                "",
                style!().fg(note.kind.colour()),
                "",
                style!().fg(ansi::RESET),
            )?;
        }

        return Ok(string);
    }

    // pub fn show<'a>(
    //     &self,
    //     f: &mut std::fmt::Formatter<'_>,
    //     module_resolver: impl Fn(ModuleId) -> Option<&'a Module>,
    // ) -> std::fmt::Result {
    //     let bold = style!(bold);

    //     write!(f, "{bold}")?;

    //     let module = self.ctx.module.as_ref().map(|m| {
    //         module_resolver(*m).unwrap_or_else(|| unreachable!("unknown module in diagnostic: {m}"))
    //     });

    //     match module {
    //         Some(module) => write!(f, "{}:", module.path_str())?,
    //         None => write!(f, "{}:", "stdin")?,
    //     }

    //     match &self.ctx.pos {
    //         Some(pos) => writeln!(f, "{}:{}: ", pos.line(), pos.col())?,
    //         None => writeln!(f, "")?,
    //     }

    //     write!(f, "{}", bold.inverse())?;

    //     writeln!(
    //         f,
    //         "  {} {}",
    //         styled!(self.kind.as_str(), bold).fg(self.kind.colour()),
    //         self.diagnostic
    //     )?;

    //     let (Some(pos), Some(module)) = (&self.ctx.pos, module) else {
    //         return Ok(());
    //     };

    //     let line_no_length = f64::log10(pos.line() as f64) as usize + 2;
    //     let start_padding = usize::max(line_no_length, 4);

    //     (0..start_padding).try_for_each(|_| write!(f, " "))?;

    //     writeln!(f, "{}", "| ".fg(ansi::BLUE))?;

    //     (0..(start_padding - line_no_length)).try_for_each(|_| write!(f, " "))?;

    //     writeln!(
    //         f,
    //         "{}{}{} |{}{} {}{}{}",
    //         bold,
    //         ansi::fg(ansi::BLUE),
    //         pos.line(),
    //         bold.inverse(),
    //         ansi::fg(ansi::RESET),
    //         &source[line_start..pos.start()],
    //         &source[pos.start()..pos.start() + pos.span()].fg(ansi::RED),
    //         &source[pos.start() + pos.span()..line_end],
    //     )?;

    //     (0..start_padding).try_for_each(|_| write!(f, " "))?;

    //     write!(f, "{}", "| ".fg(ansi::BLUE))?;

    //     (0..pos.start() - line_start).try_for_each(|_| write!(f, " "))?;

    //     write!(f, "{}", ansi::fg(ansi::RED))?;
    //     (0..pos.span()).try_for_each(|_| write!(f, "^"))?;
    //     writeln!(f, "{}", ansi::fg(ansi::RESET))?;

    //     Ok(())
    // }
}

#[derive(Default, Debug)]
pub struct Diagnostics {
    diagnostics: Vec<Diagnostic>,
    info_count: usize,
    warning_count: usize,
    error_count: usize,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        match diagnostic.kind {
            DiagnosticKind::Info => self.info_count += 1,
            DiagnosticKind::Warning => self.warning_count += 1,
            DiagnosticKind::Error => self.error_count += 1,
        }
        self.diagnostics.push(diagnostic);
    }

    pub fn append<T>(&mut self, d: T)
    where
        T: IntoIterator<Item = Diagnostic>,
    {
        for diagnostic in d {
            self.push(diagnostic);
        }
    }

    pub fn iter(&self) -> DiagnosticsIter<'_> {
        DiagnosticsIter {
            diagnostics: &self.diagnostics,
            index: 0,
        }
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl From<Diagnostic> for Diagnostics {
    fn from(diagnostic: Diagnostic) -> Self {
        let mut diagnostics = Diagnostics::new();
        diagnostics.push(diagnostic);
        diagnostics
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

impl FromIterator<Diagnostic> for Diagnostics {
    fn from_iter<T: IntoIterator<Item = Diagnostic>>(iter: T) -> Self {
        let mut diagnostics = Diagnostics::new();
        for diagnostic in iter {
            diagnostics.push(diagnostic);
        }
        diagnostics
    }
}

pub struct DiagnosticsIter<'a> {
    diagnostics: &'a [Diagnostic],
    index: usize,
}

impl<'a> Iterator for DiagnosticsIter<'a> {
    type Item = &'a Diagnostic;

    fn next(&mut self) -> Option<Self::Item> {
        self.diagnostics.get(self.index).map(|d| {
            self.index += 1;
            d
        })
    }
}

pub use diagnostic;
