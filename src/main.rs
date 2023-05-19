use std::cell::Cell;

#[derive(Debug, Copy, Clone)]
enum Register {
    Addressable(AddressableRegister),
    Bak,
}

#[derive(Debug, Copy, Clone)]
enum AddressableRegister {
    Acc,
    Nil,
}

#[derive(Debug, Copy, Clone)]
enum Port {
    P1,
    P2,
    P3,
    P4,
}

#[derive(Debug, Copy, Clone)]
enum PsuedoPort {
    Any(Port),
    Last(Port),
}

#[derive(Debug, Copy, Clone)]
enum Src {
    Port(Port),
    Register(AddressableRegister),
    Literal(i16),
}

#[derive(Debug, Copy, Clone)]
enum Dst {
    Port(Port),
    Register(AddressableRegister),
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Mode {
    Run,
    Read,
    Write,
}

#[derive(Debug, Copy, Clone)]
enum Instruction {
    Add(Src),
    Sub(Src),
    Mov(Src, Dst),
    Sav,
    Swp,
    Neg,
    Jro(Src),
    Jez(Src),
    Jnz(Src),
    Jgz(Src),
    Jlz(Src),
    Hcf,
}

impl Instruction {
    fn get_read_src(&self) -> Option<Src> {
        match self {
            Self::Mov(s, _)
            | Self::Sub(s)
            | Self::Add(s)
            | Self::Jro(s)
            | Self::Jez(s)
            | Self::Jnz(s)
            | Self::Jgz(s)
            | Self::Jlz(s) => Some(*s),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Channel {
    value: Cell<Option<i16>>,
}

impl Channel {
    fn new() -> Self {
        Self {
            value: Cell::new(None),
        }
    }
}

#[derive(Debug)]
struct Node<'a> {
    acc: i16,
    bak: i16,
    instruction_pointer: u8,
    instructions: [Option<Instruction>; 255],
    port_1: Option<&'a Channel>,
    port_2: Option<&'a Channel>,
    port_3: Option<&'a Channel>,
    port_4: Option<&'a Channel>,
    port_buffer: Option<i16>,
    mode: Mode,
}

impl Node<'_> {
    fn new() -> Self {
        Self {
            acc: 0,
            bak: 0,
            instruction_pointer: 0,
            instructions: [None; 255],
            port_1: None,
            port_2: None,
            port_3: None,
            port_4: None,
            port_buffer: None,
            mode: Mode::Run,
        }
    }
    fn map_port(&self, port: Port) -> Option<&Channel> {
        match port {
            Port::P1 => self.port_1,
            Port::P2 => self.port_2,
            Port::P3 => self.port_3,
            Port::P4 => self.port_4,
        }
    }
    fn read_prestep(&mut self) {
        // Before we do anything else, we need to resolve all read scenarios
        let instruction_maybe = self.instructions[self.instruction_pointer as usize];
        if let Some(instruction) = instruction_maybe {
            if let Some(src) = instruction.get_read_src() {
                self.handle_reads(src);
            }
        }
    }
    fn handle_reads(&mut self, src: Src) {
        if let Src::Port(port) = src {
            // If we aren't dealing with a Port as the source, it isn't a read
            if Mode::Run == self.mode {
                // If we aren't already reading or writing, assume for now that this is a read
                self.mode = Mode::Read;
                // Reading is a two step operation, now that we have signaled intent, return early
                return;
            }
            let target_port = self.map_port(port);
            if let Some(channel) = target_port {
                if self.port_buffer.is_none() {
                    // do not rewrite the port buffer, we may have already read a value
                    // from here. For instance, if we are doing a read/write mov
                    self.port_buffer = channel.value.take();
                }
            } else {
                panic!("unconnected port read attempt");
            }
        }
    }
    fn step(&mut self) {
        // Now that reads are resolved, we can continue with all other instructions
        let instruction = self.instructions[self.instruction_pointer as usize];
        match instruction {
            Some(Instruction::Mov(src, dst)) => self.mov(src, dst),
            Some(Instruction::Add(src)) => self.add(src),
            Some(Instruction::Sav) => self.sav(),
            Some(Instruction::Swp) => self.swp(),
            None => self.instruction_pointer = 0,
            _ => unimplemented!(),
        };
        if self.mode == Mode::Run {
            self.instruction_pointer += 1;
            if self.instruction_pointer > 254 {
                self.instruction_pointer = 0;
            }
        }
    }
    fn mov(&mut self, src: Src, dst: Dst) {
        let val = match src {
            Src::Port(_) => {
                if self.port_buffer.is_some() && self.mode != Mode::Write {
                    // our read was successful so we reset mode
                    self.mode = Mode::Run;
                }
                self.port_buffer
            }
            Src::Register(register) => match register {
                AddressableRegister::Acc => Some(self.acc),
                AddressableRegister::Nil => Some(0_i16),
            },
            Src::Literal(v) => Some(v),
        };
        if val.is_none() {
            return;
        }
        match dst {
            Dst::Port(port) => {
                let target_port = self.map_port(port);
                if let Some(channel) = target_port {
                    if self.mode == Mode::Write && channel.value.get().is_none() {
                        self.mode = Mode::Run;
                        self.port_buffer = None;
                    } else {
                        channel.value.set(val);
                        self.mode = Mode::Write;
                    }
                } else {
                    panic!("unconnected port write attempt");
                }
            }
            Dst::Register(register) => {
                self.port_buffer = None;
                match register {
                    AddressableRegister::Acc => self.acc = val.unwrap(),
                    AddressableRegister::Nil => (),
                }
            }
        };
    }
    fn add(&mut self, src: Src) {
        match src {
            Src::Port(port) => {
                if self.port_buffer.is_some() {
                    // our read was successful so we reset mode
                    self.mode = Mode::Run;
                    self.acc = self.acc.saturating_add(self.port_buffer.unwrap());
                }
            }
            Src::Register(register) => {
                match register {
                    AddressableRegister::Acc => {
                        self.acc = self.acc.saturating_add(self.acc);
                    }
                    AddressableRegister::Nil => (),
                };
            }
            Src::Literal(val) => self.acc = self.acc.saturating_add(val),
        };
    }
    fn swp(&mut self) {
        std::mem::swap(&mut self.bak, &mut self.acc);
    }
    fn sav(&mut self) {
        self.bak = self.acc;
    }
}

fn main() {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_add() {
        let mut node = Node::new();
        node.instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node.instructions[1] = Some(Instruction::Add(Src::Register(AddressableRegister::Acc)));
        node.read_prestep();
        node.step();
        assert_eq!(42, node.acc);
        node.read_prestep();
        node.step();
        assert_eq!(84, node.acc);
    }

    #[test]
    fn basic_sav() {
        let mut node = Node::new();
        node.instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node.instructions[1] = Some(Instruction::Sav);
        node.read_prestep();
        node.step();
        node.read_prestep();
        node.step();
        assert_eq!(42, node.bak);
    }

    #[test]
    fn basic_swp() {
        let mut node = Node::new();
        node.instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node.instructions[1] = Some(Instruction::Sav);
        node.instructions[2] = Some(Instruction::Mov(Src::Literal(13), Dst::Register(AddressableRegister::Acc)));
        node.instructions[3] = Some(Instruction::Swp);
        node.read_prestep();
        node.step();
        node.read_prestep();
        node.step();
        node.read_prestep();
        node.step();
        node.read_prestep();
        node.step();
        assert_eq!(13, node.bak);
        assert_eq!(42, node.acc);
    }

    fn plane_step(nodes: &mut [Node]) {
        for node in nodes.iter_mut() {
            node.read_prestep();
        }
        for node in nodes.iter_mut() {
            node.step();
        }
    }

    #[test]
    fn basic_port_mov() {
        let mut node1 = Node::new();
        let mut node2 = Node::new();
        let channels = vec![Channel::new()];
        node1.port_1 = Some(&channels[0]);
        node2.port_1 = Some(&channels[0]);
        node1.instructions[0] = Some(Instruction::Mov(Src::Literal(42), Dst::Port(Port::P1)));
        node2.instructions[0] = Some(Instruction::Mov(
            Src::Port(Port::P1),
            Dst::Register(AddressableRegister::Acc),
        ));
        node1.instructions[1] = Some(Instruction::Mov(
            Src::Port(Port::P1),
            Dst::Register(AddressableRegister::Acc),
        ));
        node2.instructions[1] = Some(Instruction::Mov(Src::Literal(13), Dst::Port(Port::P1)));
        let mut nodes = [node1, node2];
        plane_step(&mut nodes);
        assert_eq!(Mode::Write, nodes[0].mode);
        assert_eq!(Mode::Read, nodes[1].mode);
        plane_step(&mut nodes);
        assert_eq!(Mode::Run, nodes[0].mode);
        assert_eq!(Mode::Run, nodes[1].mode);
        assert_eq!(42, nodes[1].acc);
        plane_step(&mut nodes);
        assert_eq!(Mode::Read, nodes[0].mode);
        assert_eq!(Mode::Write, nodes[1].mode);
        plane_step(&mut nodes);
        assert_eq!(Mode::Run, nodes[0].mode);
        assert_eq!(Mode::Run, nodes[1].mode);
        assert_eq!(13, nodes[0].acc);
    }

    #[test]
    fn port_mov_back() {
        let mut node1 = Node::new();
        let mut node2 = Node::new();
        let channels = vec![Channel::new()];
        node1.port_1 = Some(&channels[0]);
        node2.port_1 = Some(&channels[0]);
        node1.instructions[0] = Some(Instruction::Mov(Src::Literal(13), Dst::Port(Port::P1)));
        node2.instructions[0] = Some(Instruction::Mov(Src::Port(Port::P1), Dst::Port(Port::P1)));
        node1.instructions[1] = Some(Instruction::Mov(
            Src::Port(Port::P1),
            Dst::Register(AddressableRegister::Acc),
        ));
        node2.instructions[1] = Some(Instruction::Add(Src::Register(AddressableRegister::Nil))); // nop
        node1.instructions[2] = Some(Instruction::Hcf);
        node2.instructions[2] = Some(Instruction::Hcf);
        let mut nodes = [node1, node2];
        plane_step(&mut nodes);
        assert_eq!(Mode::Write, nodes[0].mode);
        assert_eq!(Mode::Read, nodes[1].mode);
        plane_step(&mut nodes);
        assert_eq!(Mode::Run, nodes[0].mode);
        assert_eq!(Mode::Write, nodes[1].mode);
        plane_step(&mut nodes);
        assert_eq!(Mode::Read, nodes[0].mode);
        assert_eq!(Mode::Write, nodes[1].mode);
        plane_step(&mut nodes);
        assert_eq!(Mode::Run, nodes[0].mode);
        assert_eq!(Mode::Run, nodes[1].mode);
        assert_eq!(13, nodes[0].acc);
    }

    #[test]
    fn port_mov_three_nodes() {
        let mut node1 = Node::new();
        let mut node2 = Node::new();
        let mut node3 = Node::new();
        let channels = vec![Channel::new(), Channel::new()];
        node1.port_1 = Some(&channels[0]);
        node2.port_1 = Some(&channels[0]);
        node2.port_2 = Some(&channels[1]);
        node3.port_1 = Some(&channels[1]);
        node1.instructions[0] = Some(Instruction::Mov(Src::Literal(42), Dst::Port(Port::P1)));
        node2.instructions[0] = Some(Instruction::Mov(Src::Port(Port::P1), Dst::Port(Port::P2)));
        node3.instructions[0] = Some(Instruction::Mov(
            Src::Port(Port::P1),
            Dst::Register(AddressableRegister::Acc),
        ));
        node1.instructions[1] = Some(Instruction::Add(Src::Register(AddressableRegister::Nil)));
        let mut nodes = [node1, node2, node3];
        plane_step(&mut nodes);
        plane_step(&mut nodes);
        plane_step(&mut nodes);
        assert_eq!(42, nodes[2].acc);
        for node in nodes {
            assert_eq!(Mode::Run, node.mode)
        }
    }
}
