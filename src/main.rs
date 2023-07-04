use std::io::Read;

#[derive(Debug, Eq, PartialEq)]
enum Opcode {
    Add(u8),
    /// Open
    BranchZero(i32),
    /// Close
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

fn to_basic_opcode(c: u8) -> Option<BasicOpcode> {
    match c {
        b'+' => Some(BasicOpcode::Add),
        b'-' => Some(BasicOpcode::Sub),
        b'>' => Some(BasicOpcode::Right),
        b'<' => Some(BasicOpcode::Left),
        b'[' => Some(BasicOpcode::Open),
        b']' => Some(BasicOpcode::Close),
        b'.' => Some(BasicOpcode::Dot),
        b',' => Some(BasicOpcode::Comma),
        _ => None,
    }
}
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

fn match_block(iter: &mut impl Iterator<Item = BasicOpcode>) -> Vec<TreeOpcode> {
    let mut buffer: Vec<TreeOpcode> = Vec::new();

    loop {
        let Some(next) = iter.next() else { return buffer; };

        match next {
            BasicOpcode::Add => {
                if let Some(TreeOpcode::Add(prev)) = buffer.last_mut() {
                    *prev += 1;
                    if *prev == 0 {
                        buffer.pop();
                    }
                } else {
                    buffer.push(TreeOpcode::Add(1));
                }
            }
            BasicOpcode::Sub => {
                if let Some(TreeOpcode::Add(prev)) = buffer.last_mut() {
                    *prev -= 1;
                    if *prev == 0 {
                        buffer.pop();
                    }
                } else {
                    buffer.push(TreeOpcode::Add(-1));
                }
            }
            BasicOpcode::Right => {
                if let Some(TreeOpcode::Right(prev)) = buffer.last_mut() {
                    *prev += 1;
                    if *prev == 0 {
                        buffer.pop();
                    }
                } else {
                    buffer.push(TreeOpcode::Right(1));
                }
            }
            BasicOpcode::Left => {
                if let Some(TreeOpcode::Right(prev)) = buffer.last_mut() {
                    *prev -= 1;
                    if *prev == 0 {
                        buffer.pop();
                    }
                } else {
                    buffer.push(TreeOpcode::Right(-1));
                }
            }
            BasicOpcode::Dot => buffer.push(TreeOpcode::Dot),
            BasicOpcode::Comma => buffer.push(TreeOpcode::Comma),
            BasicOpcode::Open => buffer.push(TreeOpcode::Loop(match_block(iter))),
            BasicOpcode::Close => return buffer,
        };
    }
}

fn generate_opcodes_direct(
    mut iter: impl Iterator<Item = BasicOpcode>,
) -> Result<Vec<Opcode>, &'static str> {
    let mut buffer = Vec::new();
    use Opcode::*;

    let mut open_stack: Vec<usize> = Vec::new();
    loop {
        let Some(opcode) = iter.next() else { break };

        match opcode {
            BasicOpcode::Add => {
                if let Some(Add(p)) = buffer.last_mut() {
                    *p = p.wrapping_add(1)
                } else {
                    buffer.push(Add(1))
                }
            }
            BasicOpcode::Sub => {
                if let Some(Add(p)) = buffer.last_mut() {
                    *p = p.wrapping_sub(1)
                } else {
                    buffer.push(Add(-1_i32 as _))
                }
            }
            BasicOpcode::Right => {
                if let Some(Right(p)) = buffer.last_mut() {
                    *p = p.wrapping_add(1)
                } else {
                    buffer.push(Right(1))
                }
            }
            BasicOpcode::Left => {
                if let Some(Right(p)) = buffer.last_mut() {
                    *p = p.wrapping_sub(1)
                } else {
                    buffer.push(Right(-1 as _))
                }
            }
            BasicOpcode::Open => {
                open_stack.push(buffer.len());
                buffer.push(BranchZero(0));
            }
            BasicOpcode::Close => {
                let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                let this = buffer.len();
                let diff = (this - other) as i32;
                buffer.push(BranchNotZero(-diff));
                buffer[other] = BranchZero(diff);
            }
            BasicOpcode::Dot => buffer.push(Dot),
            BasicOpcode::Comma => buffer.push(Comma),
        }
    }
    if !open_stack.is_empty() {
        return Err("unbalanced brackets: extra [");
    } else {
        Ok(buffer)
    }
}

fn main() -> Result<(), &'static str> {
    let mut args = std::env::args().skip(1);

    let basic_opcodes_iter = std::fs::File::open(args.next().ok_or("input file missing")?)
        .map_err(|_| "could not open file")?
        .bytes()
        .filter_map(Result::ok)
        .filter_map(to_basic_opcode);

    let input_args = args.next();
    let input = input_args
        .as_ref()
        .map(String::as_str)
        .unwrap_or("")
        .as_bytes();

    let program_basic_opcodes: Vec<_> = basic_opcodes_iter.collect();

    let data_len = 1_usize << 24;

    let opcodes = generate_opcodes_direct(program_basic_opcodes.iter().copied())?;

    println!("allocating buffer...");
    let mut data: Vec<u8> = (0..data_len).map(|_| 0).collect();
    println!("allocation done...");

    let start = std::time::Instant::now();
    execute(&opcodes, &mut data, input);
    println!("elapsed: {:?}", start.elapsed());

    Ok(())
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
