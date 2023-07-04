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
    Clear,
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

fn generate_opcodes(
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
                buffer[other] = BranchZero(diff);

                if let &[.., BranchZero(_), Add(255)] = &buffer[..] {
                    buffer.truncate(buffer.len() - 2);
                    buffer.push(Clear);
                }
                buffer.push(BranchNotZero(-diff));
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

    let opcodes = generate_opcodes(program_basic_opcodes.iter().copied())?;

    println!("{:?}", &opcodes);

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
            Opcode::Clear => data[dp as usize] = 0,
        }
        pc += 1
    }
}
