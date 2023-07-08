use std::io::Read;
use std::io::Write;
use std::time::Instant;


#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
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

const DATA_LEN: usize = 1 << 16;

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

trait BfEngine {
    type OPCODE;
    fn generate(iter: impl Iterator<Item = BasicOpcode>) -> Vec<Self::OPCODE>;
    fn execute(opcodes: &[Opcode], data: &mut [u8; DATA_LEN], input: &[u8], stdout: &mut std::io::StdoutLock<'static>);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Opcode {
    Add(u8),
    /// Open
    BranchZero(u16),
    /// Close
    BranchNotZero(u16),
    Right(i16),
    Dot,
    Comma,

    Clear,
    AddTo(i16),
    Seek(i16),
}

enum NewOpcode {
    //Add(u8),
    //Right(i16),
    AddRight(u8, i16),

    //Clear,
    //AddTo(i16),
    //Set(u8),
    SetAddTo(u8, i16), 

    Seek(i16),
    
    BranchZero(u16),
    BranchNotZero(u16),
    Dot,
    Comma,
}

fn generate_opcodes(
    mut iter: impl Iterator<Item = BasicOpcode>,
) -> Result<Vec<Opcode>, &'static str> {
    let mut buffer = Vec::new();
    use Opcode::*;
    let mut toplevel = false;
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
                open_stack.push(buffer.len());
                buffer.push(BranchZero(0));
                toplevel = true;
            }
            BasicOpcode::Close => {
                let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                let this = buffer.len();
                buffer[other] = BranchZero(this.try_into().unwrap());

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
                        buffer.push(BranchNotZero(other.try_into().unwrap()));
                        if toplevel {
                            println!("{}", opcodes_to_string(&buffer[other..]));
                        }
                    }
                }
                toplevel = false;
            }
            BasicOpcode::Dot => buffer.push(Dot),
            BasicOpcode::Comma => buffer.push(Comma),
        }
        match &buffer[..] {
            &[.., Add(0)] | &[.., Right(0)] | &[.., AddTo(_), Clear] => {
                println!("found redundant code {:?}", buffer);
                let _ = buffer.pop();
            }
            _ => (),
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

    let input = args.next().unwrap_or(String::new());

    let opcodes: Vec<_> = basic_opcodes_iter.collect();

    let basic_opcodes_iter = opcodes.iter().copied();

    let start = Instant::now();
    println!("generating opcodes...");
    let opcodes = generate_opcodes(basic_opcodes_iter)?;
    println!("generating done in {:?}", start.elapsed());

    {
        let mut stats = std::collections::HashMap::new();
        let opcodes: Vec<_> = opcodes
            .iter()
            .map(|opcode| match opcode {
                Opcode::Add(_) => '+',
                Opcode::BranchZero(_) => '[',
                Opcode::BranchNotZero(_) => ']',
                Opcode::Right(_) => '>',
                Opcode::Dot => '.',
                Opcode::Comma => ',',
                Opcode::Clear => 'C',
                Opcode::AddTo(_) => 'M',
                Opcode::Seek(_) => 'S',
            })
            .collect();
        for w in opcodes.windows(2) {
            *stats.entry(w).or_insert(0) += 1
        }
        let mut stats: Vec<_> = stats.iter().map(|(a, b)| (*a, *b)).collect();
        stats.sort_by_key(|s| s.1);
        dbg!(stats);
    }

    let start = Instant::now();
    println!("allocating buffer...");
    let mut data: Box<[u8; DATA_LEN]> = alloc_const();
    println!("allocation done in {:?}", start.elapsed());

    println!("{}", opcodes_to_string(&opcodes));
    println!("{} instructions generated", opcodes.len());

    let start = Instant::now();
    let stdout = &mut std::io::stdout().lock();
    execute(&opcodes, &mut data, input.as_bytes(), stdout);
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
fn execute(
    opcodes: &[Opcode],
    data: &mut [u8; DATA_LEN],
    input: &[u8],
    stdout: &mut std::io::StdoutLock<'static>,
) {
    let mut input = input.iter().copied();
    let mut pc: usize = 0;
    let mut dp: usize = 0;
    loop {
        let Some(opcode) = opcodes.get(pc) else { break };

        match opcode {
            Opcode::Add(i) => {
                data[dp] = data[dp].wrapping_add(*i);
            }
            Opcode::BranchZero(i) => {
                if data[dp] == 0 {
                    pc = *i as _;
                }
            }
            Opcode::BranchNotZero(i) => {
                if data[dp] != 0 {
                    pc = *i as _;
                }
            }
            Opcode::Right(i) => {
                dp = dp.wrapping_add(*i as _);
            }
            Opcode::Dot => {
                let _ = stdout.write(&[data[dp]]);
            }
            Opcode::Comma => {
                data[dp as usize] = input.next().unwrap();
            }
            Opcode::Clear => data[dp as usize] = 0,
            Opcode::AddTo(i) => {
                let from = dp as usize;
                let to = ((dp.wrapping_add(*i as _)) as usize) % DATA_LEN;

                let tmp = data[from];

                data[from] = tmp.wrapping_sub(tmp);
                data[to] = data[to].wrapping_add(tmp);
            }
            Opcode::Seek(i) => {
                while data[dp as usize] != 0 {
                    dp = dp.wrapping_add(*i as _);
                }
            }
        }
        pc += 1
    }
}
