use std::io::Read;

#[derive(Debug, Eq, PartialEq)]
enum Opcode {
    Add(u8),
    /// Open
    BranchZero(u32),
    /// Close
    BranchNotZero(u32),
    Right(i32),
    Dot,
    Comma,

    Clear,
    AddTo(i32),
    Seek(i32),
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

const DATA_LEN: usize = 1 << 20;

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
    let mut top_level_loops: Vec<String> = Vec::new();

    let mut buffer = Vec::new();
    use Opcode::*;

    let mut toplevel = true;
    let mut open_stack: Vec<usize> = Vec::new();
    loop {
        let Some(opcode) = iter.next() else { break };

        match opcode {
            BasicOpcode::Add => {
                if let Some(Add(p)) = buffer.last_mut() {
                    *p = p.wrapping_add(1);
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
                toplevel = true;
                open_stack.push(buffer.len());
                buffer.push(BranchZero(0));
            }
            BasicOpcode::Close => {
                let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                let this = buffer.len();
                buffer[other] = BranchZero(this as u32);

                match &buffer[..] {
                    &[.., BranchZero(_), Add(255)] => {
                        buffer.truncate(buffer.len() - 2);
                        buffer.push(Clear);
                    }
                    &[.., BranchZero(_), Add(255), Right(x), Add(1), Right(y)] if -x == y => {
                        buffer.truncate(buffer.len() - 5);
                        buffer.push(AddTo(x));
                    }
                    &[.., BranchZero(_), Right(x)] => {
                        buffer.push(Seek(x));
                    }
                    _ => {
                        //buffer.push(BranchNotZero(-diff));
                        buffer.push(BranchNotZero(other as u32));
                        if toplevel {
                            top_level_loops.push(opcodes_to_string(&buffer[other..=this]));
                        }
                    }
                }
                toplevel = false;
            }
            BasicOpcode::Dot => buffer.push(Dot),
            BasicOpcode::Comma => buffer.push(Comma),
        }
        if let Some(Add(0) | Right(0)) = buffer.last() {
            let _ = buffer.pop();
        }
    }
    top_level_loops.sort();
    dbg!(top_level_loops);
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

    //println!("{:?}", &opcodes);

    println!("allocating buffer...");
    let mut data: Vec<u8> = (0..data_len).map(|_| 0).collect();
    println!("allocation done...");

    println!("{}", opcodes_to_string(&opcodes));

    let start = std::time::Instant::now();
    //execute(&opcodes, &mut data, input);
    unsafe { execute_unsafe(&opcodes, &mut data, input) };
    println!("elapsed: {:?}", start.elapsed());

    Ok(())
}

fn alloc_const<const LEN: usize>() -> Box<[u8; LEN]> {
    (0..LEN)
        .map(|_| 0_u8)
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

fn opcodes_to_string(opcodes: &[Opcode]) -> String {
    use std::fmt::Write;
    let mut buffer = String::new();
    for opcode in opcodes.iter() {
        match opcode {
            Opcode::Seek(i) => {
                let _ = write!(&mut buffer, "S({i})");
            }
            Opcode::AddTo(i) => {
                let _ = write!(&mut buffer, "M({i})");
            }
            Opcode::Add(i) => {
                let i = *i as i8;
                if i > 0 {
                    let _ = write!(&mut buffer, "+{}", i as i32);
                } else {
                    let _ = write!(&mut buffer, "-{}", -(i as i32));
                }
            }
            Opcode::BranchZero(_) => buffer.push('['),
            Opcode::BranchNotZero(_) => buffer.push(']'),
            Opcode::Right(i) => {
                let i = *i;
                if i > 0 {
                    let _ = write!(&mut buffer, ">{}", i as i32);
                } else {
                    let _ = write!(&mut buffer, "<{}", -(i as i32));
                }
            }
            Opcode::Dot => buffer.push('.'),
            Opcode::Comma => buffer.push(','),
            Opcode::Clear => buffer.push('C'),
        }
    }
    buffer
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
                    pc = *i as _;
                    continue;
                }
            }
            Opcode::BranchNotZero(i) => {
                if data[dp as usize] != 0 {
                    pc = *i as _;
                    continue;
                }
            }
            Opcode::Right(i) => {
                dp = dp.wrapping_add(*i as _);
            }
            Opcode::Dot => {
                print!("{}", char::from_u32(data[dp as usize].into()).unwrap());
            }
            Opcode::Comma => {
                data[dp as usize] = input.next().unwrap();
            }
            Opcode::Clear => data[dp as usize] = 0,
            Opcode::AddTo(i) => {
                let from = dp as usize;
                let to = (dp + i) as usize;

                if data[from] != 0 {
                    let tmp = data[from];
                    data[from] = data[from].wrapping_sub(tmp);
                    data[to] = data[to].wrapping_add(tmp);
                }
            }
            Opcode::Seek(i) => {
                while data[dp as usize] != 0 {
                    dp += *i;
                }
            }
        }
        pc += 1
    }
}

#[inline(never)]
unsafe fn execute_unsafe(opcodes: &[Opcode], data: &mut [u8], input: &[u8]) {
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
                    pc = *i as _;
                    continue;
                }
            }
            Opcode::BranchNotZero(i) => {
                if data[dp as usize] != 0 {
                    pc = *i as _;
                    continue;
                }
            }
            Opcode::Right(i) => {
                dp = dp.wrapping_add(*i as _);
            }
            Opcode::Dot => {
                print!("{}", char::from_u32(data[dp as usize].into()).unwrap());
            }
            Opcode::Comma => {
                data[dp as usize] = input.next().unwrap();
            }
            Opcode::Clear => data[dp as usize] = 0,
            Opcode::AddTo(i) => {
                let from = dp as usize;
                let to = (dp + i) as usize;

                if data[from] != 0 {
                    let tmp = data[from];
                    data[from] = data[from].wrapping_sub(tmp);
                    data[to] = data[to].wrapping_add(tmp);
                }
            }
            Opcode::Seek(i) => {
                while data[dp as usize] != 0 {
                    dp += *i;
                }
            }
        }
        pc += 1;
        if opcodes.len() == pc as _ {
            break;
        }
    }
}
