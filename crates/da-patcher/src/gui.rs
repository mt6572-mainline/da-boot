use std::{
    env::current_dir,
    fs,
    num::ParseIntError,
    ops::RangeInclusive,
    path::PathBuf,
    sync::{
        LazyLock,
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use da_patcher::{
    Assembler, Disassembler, Patch, PatchCollection, Result, err::Error, preloader::Preloader,
    slice::fuzzy::generic_reg_matcher,
};
use derive_ctor::ctor;
use derive_more::IsVariant;
use eframe::egui::{
    self, Color32, Key, RichText, ScrollArea, SidePanel, TextEdit, TextStyle, Window,
};
use egui_file::FileDialog;

static CWD: LazyLock<PathBuf> = LazyLock::new(|| current_dir().unwrap());

#[derive(ctor, Debug)]
struct Instruction {
    offset: usize,
    instr: (String, String),
}

#[derive(ctor)]
struct Code {
    code: Vec<u8>,
    instructions: Vec<Instruction>,
}

#[derive(Default, IsVariant)]
enum State {
    #[default]
    Start,
    FileDialog(FileDialog),
    WaitForWorker,
    Disassembly(Code, String),
}

#[derive(Default)]
struct GotoWindow {
    open: bool,
    focus: bool,
    edit: String,
    err: Option<ParseIntError>,
}

#[derive(Default)]
struct PatchWindow {
    open: bool,
    err: Option<da_patcher::err::Error>,
}

#[derive(ctor)]
struct App<'a> {
    state: State,
    tx: Sender<Code>,
    rx: Receiver<Code>,
    scroll_to: Option<usize>,
    highlight: Option<RangeInclusive<usize>>,
    goto: GotoWindow,
    patch_window: PatchWindow,

    asm: Assembler,
    disasm: Disassembler<'a>,
}

impl<'a> eframe::App for App<'a> {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if self.state.is_disassembly() {
            let width = ctx.available_rect().width() / 2.;
            SidePanel::left("disassembly_view")
                .min_width(width)
                .show(ctx, |ui| {
                    if let &mut State::Disassembly(ref code, ref mut pattern) = &mut self.state {
                        let vec = &code.instructions;
                        ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                let rect = ui.max_rect();
                                let row_height = ui.text_style_height(&TextStyle::Monospace);
                                let visible_range = ui.clip_rect().y_range();

                                if let Some(to) = self.scroll_to {
                                    let target_y = vec.iter().position(|i| i.offset == to).unwrap()
                                        as f32
                                        * row_height;

                                    let target_rect = egui::Rect::from_min_max(
                                        rect.left_top() + egui::vec2(0.0, target_y),
                                        rect.left_top() + egui::vec2(0.0, target_y + row_height),
                                    );

                                    ui.scroll_to_rect(target_rect, Some(egui::Align::Center));
                                    self.scroll_to = None;
                                }

                                let total_height = vec.len() as f32 * row_height;

                                let mut first = ((visible_range.min - rect.top()) / row_height)
                                    .floor()
                                    as usize;
                                let mut last =
                                    ((visible_range.max - rect.top()) / row_height).ceil() as usize;

                                first = first.clamp(0, vec.len());
                                last = last.clamp(0, vec.len());

                                ui.set_min_height(total_height);

                                let start_pos =
                                    rect.left_top() + egui::vec2(0.0, first as f32 * row_height);

                                ui.allocate_ui_at_rect(
                                    egui::Rect::from_min_max(
                                        start_pos,
                                        rect.left_top()
                                            + egui::vec2(rect.width(), last as f32 * row_height),
                                    ),
                                    |ui| {
                                        for instr in vec[first..last].iter() {
                                            let text = if let Some(highlight) = &mut self.highlight
                                                && highlight.contains(&instr.offset)
                                            {
                                                RichText::new(format!(
                                                    "{:08X}: {} {}",
                                                    instr.offset, instr.instr.0, instr.instr.1
                                                ))
                                                .color(Color32::GREEN)
                                            } else {
                                                RichText::new(format!(
                                                    "{:08X}: {} {}",
                                                    instr.offset, instr.instr.0, instr.instr.1
                                                ))
                                            };

                                            ui.label(text);
                                        }
                                    },
                                );
                            });

                        SidePanel::right("pattern")
                            .min_width(width)
                            .show(ctx, |ui| {
                                ui.centered_and_justified(|ui| {
                                    let edit = ui.add(
                                        TextEdit::multiline(pattern)
                                            .hint_text("Pattern")
                                            .code_editor(),
                                    );
                                    if ui.input(|i| i.key_pressed(Key::P)) && !edit.has_focus() {
                                        self.patch_window.open = true;
                                    }

                                    if !pattern.is_empty() {
                                        if let Ok((offset, end)) = search_pattern(pattern, &vec) {
                                            self.scroll_to = Some(offset);
                                            self.highlight = Some(offset..=end);
                                        } else {
                                            self.scroll_to = None;
                                            self.highlight = None;
                                        }
                                    }
                                });
                            });

                        if self.goto.open {
                            Window::new("Goto").show(ctx, |ui| {
                                ui.vertical_centered(|ui| {
                                    let input = ui.add(
                                        TextEdit::singleline(&mut self.goto.edit).code_editor(),
                                    );

                                    if self.goto.focus {
                                        input.request_focus();
                                        self.goto.focus = false;
                                    }

                                    if input.lost_focus() {
                                        match usize::from_str_radix(&self.goto.edit, 16) {
                                            Ok(v) => {
                                                self.scroll_to = Some(v);
                                                self.goto.open = false;
                                                self.goto.err = None;
                                            }
                                            Err(e) => {
                                                self.goto.focus = true;
                                                self.goto.err = Some(e);
                                            }
                                        }
                                    }

                                    if let Some(e) = &self.goto.err {
                                        ui.label(format!("Invalid offset: {e}"));
                                    }
                                });
                            });
                        }
                        if ui.input(|i| i.key_pressed(Key::G)) {
                            self.goto.open = true;
                            self.goto.focus = true;
                        }

                        if self.patch_window.open {
                            Window::new("Patches").show(ctx, |ui| {
                                ui.vertical_centered(|ui| {
                                    Preloader::all(&self.asm, &self.disasm)
                                        .into_iter()
                                        .for_each(|p| {
                                            if ui.button(p.name()).clicked() {
                                                match p.offset(&code.code) {
                                                    Ok(o) => {
                                                        let replacement =
                                                            p.replacement(&code.code).unwrap();
                                                        let disassembled =
                                                            disassemble_thumb2(replacement);
                                                        self.scroll_to = Some(o);
                                                        self.patch_window.err = None;
                                                        self.highlight = Some(
                                                            o..=o + disassembled.code.len() - 1,
                                                        );
                                                        self.patch_window.open = false;
                                                    }
                                                    Err(e) => self.patch_window.err = Some(e),
                                                }
                                            }
                                        })
                                });

                                if let Some(e) = &self.patch_window.err {
                                    ui.label(format!("Failed to find patch: {e}"));
                                }
                            });
                        }
                    }
                });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    if self.state.is_start() {
                        if ui.button("Open preloader file").clicked() {
                            let mut dialog = FileDialog::open_file(Some(CWD.clone()));
                            dialog.open();
                            self.state = State::FileDialog(dialog);
                        }
                    } else if self.state.is_wait_for_worker() {
                        ui.spinner();
                        if let Ok(instructions) = self.rx.try_recv() {
                            self.state =
                                State::Disassembly(instructions, String::with_capacity(64));
                        }
                    }
                });
                if let State::FileDialog(dialog) = &mut self.state {
                    if dialog.show(ctx).selected() {
                        if let Some(file) = dialog.path() {
                            let mut content = fs::read(file).unwrap();
                            content.truncate(131 * 1024);
                            self.state = State::WaitForWorker;

                            let tx = self.tx.clone();
                            thread::spawn(|| worker(content, tx));
                        }
                    }
                }
            });
        }
    }
}

fn search_pattern(pattern: &str, content: &Vec<Instruction>) -> Result<(usize, usize)> {
    let mut n = 0;
    let mut start = None;
    let split_instr = pattern
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim())
        .collect::<Vec<_>>();

    for i in content {
        if generic_reg_matcher(
            &i.instr.0,
            &i.instr.1,
            split_instr.get(n).ok_or(Error::PatternNotFound)?,
        )
        .is_ok_and(|b| b)
        {
            if n == 0 {
                start = Some(i.offset);
            } else {
            }

            n += 1;

            if n == split_instr.len() {
                return Ok((start.ok_or(Error::PatternNotFound)?, i.offset));
            }
        } else if n != 0 {
            n = 0;
            start = None;
        }
    }

    Err(Error::PatternNotFound)
}

fn disassemble_thumb2(content: Vec<u8>) -> Code {
    let disasm = Disassembler::try_new().unwrap();
    let mut vec = Vec::with_capacity(content.len() / 2);
    let mut offset = 0;

    while offset < content.len() {
        let insns = disasm.thumb2_disasm_count(&content[offset..], 1).unwrap();

        if let Some(insn) = insns.iter().next() {
            vec.push(Instruction::new(
                offset,
                (
                    insn.mnemonic().unwrap().to_owned(),
                    insn.op_str().unwrap().to_owned(),
                ),
            ));
            offset += insn.bytes().len();
        } else {
            // thumb2 align
            offset += 2;
        }
    }

    Code::new(content, vec)
}

fn worker(content: Vec<u8>, tx: Sender<Code>) {
    tx.send(disassemble_thumb2(content)).unwrap();
}

fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let asm = Assembler::try_new().unwrap();
    let disasm = Disassembler::try_new().unwrap();

    eframe::run_native(
        "da-patcher",
        eframe::NativeOptions::default(),
        Box::new(|_| {
            Ok(Box::new(App::new(
                State::default(),
                tx,
                rx,
                None,
                None,
                GotoWindow::default(),
                PatchWindow::default(),
                asm,
                disasm,
            )))
        }),
    )
    .map_err(Error::Eframe)
}
