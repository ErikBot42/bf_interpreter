#[derive(Debug)]
enum Opcode {
    Add(u8),
    BranchZero(i32),
    BranchNotZero(i32),
    Right(i32),
    Dot,
    Comma,
}
#[derive(Debug)]
enum TreeOpcode {
    Add(i32),
    Loop(Vec<TreeOpcode>),
    Right(i32),
    Dot,
    Comma,
}

fn main() {
    let program = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";

    let program = "
>>>>>>>>>>>
+-+-+-+-
<<<<<<<<<<<
+-+-+-+-
>>><<<


++
>+++<


[

[->>+>+<<<]
>>>
[-<<<+>>>]
<<

[->+>+<<]

>>
[-<<+>>]
<<

]

";

    //use std::io::Read;

    //fn to_basic_opcode(c: u8) -> Option<BasicOpcode> {
    //    match c {
    //        b'+' => Some(BasicOpcode::Add),
    //        b'-' => Some(BasicOpcode::Sub),
    //        b'>' => Some(BasicOpcode::Right),
    //        b'<' => Some(BasicOpcode::Left),
    //        b'[' => Some(BasicOpcode::Open),
    //        b']' => Some(BasicOpcode::Close),
    //        b'.' => Some(BasicOpcode::Dot),
    //        b',' => Some(BasicOpcode::Comma),
    //        _ => None,
    //    }
    //}

    //std::env::args()
    //    .nth(1)
    //    .ok_or("input file missing")
    //    .and_then(|filename| std::fs::File::open(filename).map_err(|_| "could not open file"))
    //    .map(Read::bytes)
    //    .map(|iter| iter.filter_map(Result::ok).filter_map(to_basic_opcode));

    println!("{program}");

    let data_len = 1_usize << 24;

    let mut input: Vec<u8> = "cbcccd\n\n".chars().map(|c| c as u8).collect();

    //let program = "+++<>---";

    #[derive(Copy, Clone)]
    enum BasicOpcode {
        Add,
        Sub,
        Open,
        Close,
        Right,
        Left,
        Dot,
        Comma,
    }
    use BasicOpcode::*;

    let program_basic_opcodes: Vec<_> = program
        .chars()
        .filter_map(|c| match c {
            '+' => Some(Add),
            '-' => Some(Sub),
            '>' => Some(Right),
            '<' => Some(Left),
            '[' => Some(Open),
            ']' => Some(Close),
            '.' => Some(Dot),
            ',' => Some(Comma),
            _ => None,
        })
        .collect();

    type I<'a> = std::iter::Copied<std::slice::Iter<'a, BasicOpcode>>;

    fn generate_opcodes(tree: &[TreeOpcode], buffer: &mut Vec<Opcode>) {
        for opcode in tree {
            match opcode {
                TreeOpcode::Add(i) => buffer.push(Opcode::Add(*i as u8)),
                TreeOpcode::Right(i) => buffer.push(Opcode::Right(*i)),
                TreeOpcode::Dot => buffer.push(Opcode::Dot),
                TreeOpcode::Comma => buffer.push(Opcode::Comma),
                TreeOpcode::Loop(v) => {
                    let index_start = buffer.len();
                    buffer.push(Opcode::BranchZero(0));
                    generate_opcodes(v, buffer);
                    let index_end = buffer.len();

                    buffer[index_start] = Opcode::BranchZero(index_end as i32 - index_start as i32);
                    buffer.push(Opcode::BranchNotZero(index_start as i32 - index_end as i32));
                }
            }
        }
    }

    fn match_block(iter: &mut I) -> Vec<TreeOpcode> {
        let mut buffer: Vec<TreeOpcode> = Vec::new();

        loop {
            let Some(next) = iter.next() else { return buffer; };

            match next {
                Add => {
                    if let Some(TreeOpcode::Add(prev)) = buffer.last_mut() {
                        *prev += 1;
                        if *prev == 0 {
                            buffer.pop();
                        }
                    } else {
                        buffer.push(TreeOpcode::Add(1));
                    }
                }
                Sub => {
                    if let Some(TreeOpcode::Add(prev)) = buffer.last_mut() {
                        *prev -= 1;
                        if *prev == 0 {
                            buffer.pop();
                        }
                    } else {
                        buffer.push(TreeOpcode::Add(-1));
                    }
                }
                Right => {
                    if let Some(TreeOpcode::Right(prev)) = buffer.last_mut() {
                        *prev += 1;
                        if *prev == 0 {
                            buffer.pop();
                        }
                    } else {
                        buffer.push(TreeOpcode::Right(1));
                    }
                }
                Left => {
                    if let Some(TreeOpcode::Right(prev)) = buffer.last_mut() {
                        *prev -= 1;
                        if *prev == 0 {
                            buffer.pop();
                        }
                    } else {
                        buffer.push(TreeOpcode::Right(-1));
                    }
                }
                Dot => buffer.push(TreeOpcode::Dot),
                Comma => buffer.push(TreeOpcode::Comma),
                Open => buffer.push(TreeOpcode::Loop(match_block(iter))),
                Close => return buffer,
            };
        }
    }

    let mut iter: I = program_basic_opcodes.iter().copied();
    let mut final_buffer = Vec::new();
    dbg!(generate_opcodes(
        &dbg!(match_block(&mut iter)),
        &mut final_buffer
    ));
    println!("{final_buffer:?}");
    let opcodes = final_buffer;

    println!("allocating buffer...");
    let mut data: Vec<u8> = (0..data_len).map(|_| 0).collect();
    println!("allocation done...");

    let start = std::time::Instant::now();
    execute(&opcodes, &mut data, &input);
    println!("elapsed: {:?}", start.elapsed());
}

#[inline(never)]
fn execute(opcodes: &[Opcode], data: &mut [u8], input: &[u8]) {
    let mut input = input.iter().copied();
    let mut pc: i32 = 0;
    let mut dp: i32 = 0;
    loop {
        let Some(opcode) = opcodes.get(pc as usize) else { break };

        match opcode {
            Opcode::Add(i) => {
                data[dp as usize] = data[dp as usize].wrapping_add(*i);
            }
            Opcode::BranchZero(i) => {
                if data[dp as usize] == 0 {
                    pc += *i;
                    continue;
                }
            }
            Opcode::BranchNotZero(i) => {
                if data[dp as usize] != 0 {
                    pc += *i;
                    continue;
                }
            }
            Opcode::Right(i) => {
                dp += *i;
            }
            Opcode::Dot => {
                print!("{}", char::from_u32(data[dp as usize].into()).unwrap());
            }
            Opcode::Comma => {
                data[dp as usize] = input.next().unwrap();
            }
        }
        pc += 1
    }
}
