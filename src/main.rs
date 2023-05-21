#[derive(Debug, Copy, Clone)]
enum Register {
    Acc,
    Nil,
    // There is also a BAK register but it is not addressable
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum TruePort {
    Up,
    Down,
    Left,
    Right,
    // originally a pseudoport, but oh well
    Any,
}

impl TruePort {
    fn reverse(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Up => Self::Down,
            Self::Right => Self::Left,
            Self::Down => Self::Up,
            Self::Any => panic!("Cannot reverse port 'Any'"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Port {
    True(TruePort),
    Last,
}

#[derive(Debug, Copy, Clone)]
enum Src {
    Port(Port),
    Register(Register),
    Literal(i16),
}

#[derive(Debug, Copy, Clone)]
enum Dst {
    Port(Port),
    Register(Register),
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
    fn get_src(&self) -> Option<Src> {
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
struct ExecutionNode {
    acc: i16,
    bak: i16,
    instruction_pointer: u8,
    current_instruction: Option<Instruction>,
    port_read_buffer: Option<i16>,
    port_write_buffer: Option<i16>,
    direction: Option<TruePort>,
    last_port: Option<TruePort>,
    mode: Mode,
}

impl ExecutionNode {
    const fn new() -> Self {
        Self {
            acc: 0,
            bak: 0,
            instruction_pointer: 0,
            current_instruction: None,
            port_read_buffer: None,
            port_write_buffer: None,
            direction: None,
            last_port: None,
            mode: Mode::Run,
        }
    }
    fn map_port(&self, port: Port) -> TruePort {
        match port {
            Port::True(p) => p,
            Port::Last => self.last_port.unwrap(),
        }
    }
    fn fetch(&mut self, instructions: &[Option<Instruction>]) {
        if let Some(instruction) = instructions[self.instruction_pointer as usize] {
            self.current_instruction = Some(instruction);
        } else {
            self.instruction_pointer = 0;
            self.current_instruction = instructions[self.instruction_pointer as usize];
        }
    }
    fn increment_instruction_pointer(&mut self) {
        self.instruction_pointer += 1;
        if self.instruction_pointer >= INSTRUCTIONS_PER_NODE as u8 {
            self.instruction_pointer = 0;
        }
    }
    fn resolve_write(&mut self) {
        // NOTE move me to a trait?
        self.mode = Mode::Run;
        self.increment_instruction_pointer();
    }
    fn read_step(&mut self) {
        if self.mode == Mode::Read || self.mode == Mode::Write {
            return;
        }
        if let Some(instruction) = self.current_instruction {
            if let Some(src) = instruction.get_src() {
                match src {
                    Src::Port(port) => {
                        self.mode = Mode::Read;
                        let p = self.map_port(port);
                        self.direction = Some(p);
                        self.last_port = Some(p)
                    }
                    _ => (),
                };
            }
        }
    }
    fn step(&mut self) {
        match self.current_instruction {
            Some(Instruction::Mov(src, dst)) => self.mov(src, dst),
            Some(Instruction::Add(src)) => self.add(src),
            Some(Instruction::Sav) => self.sav(),
            Some(Instruction::Swp) => self.swp(),
            Some(Instruction::Neg) => self.neg(),
            None => {
                return;
            },
            _ => unimplemented!(),
        };
        if self.mode == Mode::Run {
            self.increment_instruction_pointer();
        }
    }
    fn mov(&mut self, src: Src, dst: Dst) {
        let value = match src {
            Src::Port(_) => {
                if self.port_read_buffer.is_some() && self.mode != Mode::Write {
                    // our read was successful so we reset mode
                    self.mode = Mode::Run;
                }
                self.port_read_buffer
            }
            Src::Register(register) => match register {
                Register::Acc => Some(self.acc),
                Register::Nil => Some(0_i16),
            },
            Src::Literal(v) => Some(v),
        };
        if value.is_none() {
            return;
        }
        match dst {
            Dst::Port(port) => {
                if self.mode != Mode::Write {
                    self.mode = Mode::Write;
                    self.port_write_buffer = value;
                    let p = self.map_port(port);
                    self.direction = Some(p);
                    self.last_port = Some(p);
                }
            }
            Dst::Register(register) => match register {
                Register::Acc => self.acc = value.unwrap(),
                Register::Nil => (),
            },
        };
    }
    fn add(&mut self, src: Src) {
        if self.mode == Mode::Read {
            if let Some(value) = self.port_read_buffer {
                self.acc = self.acc.saturating_add(value);
                self.mode = Mode::Run;
            }
        } else {
            match src {
                Src::Register(register) => {
                    match register {
                        Register::Acc => self.acc = self.acc.saturating_add(self.acc),
                        Register::Nil => (),
                    };
                }
                Src::Literal(value) => self.acc = self.acc.saturating_add(value),
                _ => unreachable!(),
            };
        }
    }
    fn swp(&mut self) {
        std::mem::swap(&mut self.bak, &mut self.acc);
    }
    fn sav(&mut self) {
        self.bak = self.acc;
    }
    fn neg(&mut self) {
        self.acc = -self.acc;
    }
}

static NODE_LUT: [(Option<u8>, Option<u8>, Option<u8>, Option<u8>); NODES_PER_PLANE] = [
    // left up right down
    (None, None, Some(1), Some(4)),
    (Some(0), None, Some(2), Some(5)),
    (Some(1), None, Some(3), Some(6)),
    (Some(2), None, None, Some(7)),
    (None, Some(0), Some(5), Some(8)),
    (Some(4), Some(1), Some(6), Some(9)),
    (Some(5), Some(2), Some(7), Some(10)),
    (Some(6), Some(3), None, Some(11)),
    (None, Some(4), Some(9), None),
    (Some(8), Some(5), Some(10), None),
    (Some(9), Some(6), Some(11), None),
    (Some(10), Some(7), None, None),
];

static PORT_LUT: [(u8, u8, u8, u8); NODES_PER_PLANE] = [
    // left up right down
    (4, 0, 5, 9),
    (5, 1, 6, 10),
    (6, 2, 7, 11),
    (7, 3, 8, 12),
    (13, 9, 14, 18),
    (14, 10, 15, 19),
    (15, 11, 16, 20),
    (16, 12, 17, 21),
    (22, 18, 23, 27),
    (23, 19, 24, 28),
    (24, 20, 25, 29),
    (25, 21, 26, 30),
];

fn map_port(direction: TruePort, i: usize) -> usize {
    (match direction {
        TruePort::Left => PORT_LUT[i].0,
        TruePort::Up => PORT_LUT[i].1,
        TruePort::Right => PORT_LUT[i].2,
        TruePort::Down => PORT_LUT[i].3,
        TruePort::Any => unimplemented!(),
    }) as usize
}

fn reverse_map_node(direction: TruePort, i: usize) -> Option<u8> {
    match direction {
        TruePort::Left => NODE_LUT[i].0,
        TruePort::Up => NODE_LUT[i].1,
        TruePort::Right => NODE_LUT[i].2,
        TruePort::Down => NODE_LUT[i].3,
        TruePort::Any => unimplemented!(),
    }
}

trait Plane {
    fn step(&mut self) {}
}

const NODES_PER_PLANE: usize = 12;
const INSTRUCTIONS_PER_NODE: usize = 21;

struct ExecutionPlane {
    nodes: [ExecutionNode; NODES_PER_PLANE],
    ports: [Option<i16>; 31],
    queued_writes: [Option<i16>; 31],
    clear_writes: Vec<u8>,
    instructions: Box<[Option<Instruction>; NODES_PER_PLANE * INSTRUCTIONS_PER_NODE]>,
}

impl ExecutionPlane {
    fn new() -> Self {
        const NODE: ExecutionNode = ExecutionNode::new();
        Self {
            nodes: [NODE; NODES_PER_PLANE],
            ports: [None; 31],
            queued_writes: [None; 31],
            clear_writes: Vec::with_capacity(NODES_PER_PLANE),
            instructions: Box::new([None; NODES_PER_PLANE * INSTRUCTIONS_PER_NODE]),
        }
    }
    fn get_node_instructions_mut(&mut self, index: u8) -> &mut [Option<Instruction>] {
        // lil helper func
        if index >= 12 {
            panic!("12 nodes per plane, 0 indexed");
        }
        let start_offset = index as usize * INSTRUCTIONS_PER_NODE;
        let end_offset = start_offset + INSTRUCTIONS_PER_NODE;
        &mut self.instructions[start_offset..end_offset]
    }
}

impl Plane for ExecutionPlane {
    fn step(&mut self) {
        for (i, (node, instructions)) in self
            .nodes
            .iter_mut()
            .zip(self.instructions.chunks_exact(INSTRUCTIONS_PER_NODE))
            .enumerate()
        {
            node.fetch(instructions);
            if node.current_instruction.is_some() {
                println!("NODE BEFORE: {:#?}", node);
            }
            node.read_step();
            if node.mode == Mode::Read {
                if let Some(direction) = node.direction {
                    let mut port = &mut self.ports[map_port(direction, i)];
                    if port.is_some() {
                        node.port_read_buffer = port.take();
                        if let Some(index) = reverse_map_node(direction, i) {
                            self.clear_writes.push(index);
                        }
                    }
                }
            }
            node.step();
            if node.mode == Mode::Write {
                if let Some(direction) = node.direction {
                    if node.port_write_buffer.is_some() {
                        let index = map_port(direction, i);
                        self.queued_writes[index] = node.port_write_buffer.take();
                        println!("Queuing write: {:?}", self.queued_writes[index]);
                    }
                }
            }
            if node.current_instruction.is_some() {
                println!("NODE AFTER: {:#?}", node);
            }
        }
        for (i, write_maybe) in self.queued_writes.iter_mut().enumerate() {
            if write_maybe.is_some() {
                if self.ports[i].is_some() {
                    panic!("write deadlock");
                } else {
                    self.ports[i] = write_maybe.take();
                }
            }
        }
        for index in self.clear_writes.iter() {
            println!("index {index} being cleared");
            let mut node = &mut self.nodes[*index as usize];
            node.resolve_write();
        }
        self.clear_writes.clear();
    }
}

fn main() {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic(expected = "not implemented")]
    fn halt_and_catch_fire() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Hcf);
        nodeplane.step();
    }

    #[test]
    fn basic_add() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node_1_instructions[1] = Some(Instruction::Add(Src::Register(Register::Acc)));
        nodeplane.step();
        assert_eq!(42, nodeplane.nodes[0].acc);
        nodeplane.step();
        assert_eq!(84, nodeplane.nodes[0].acc);
    }

    #[test]
    fn read_add() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Port(Port::True(TruePort::Right))));
        let node_2_instructions = nodeplane.get_node_instructions_mut(1);
        node_2_instructions[0] = Some(Instruction::Mov(
            Src::Literal(5000),
            Dst::Port(Port::True(TruePort::Left))
        ));
        nodeplane.step();
        nodeplane.step();
        assert_eq!(5000, nodeplane.nodes[0].acc);
    }

    #[test]
    fn add_negative() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(-42)));
        nodeplane.step();
        assert_eq!(-42, nodeplane.nodes[0].acc);
    }

    #[test]
    fn add_saturating() {
        let max = 32767;
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(max)));
        node_1_instructions[1] = Some(Instruction::Add(Src::Literal(1)));
        nodeplane.step();
        nodeplane.step();
        assert_eq!(max, nodeplane.nodes[0].acc);
    }

    #[test]
    fn add_instruction_wraparound() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(1)));
        assert_eq!(0, nodeplane.nodes[0].acc);
        nodeplane.step();
        assert_eq!(1, nodeplane.nodes[0].acc);
        nodeplane.step();
        assert_eq!(2, nodeplane.nodes[0].acc);
        nodeplane.step();
        assert_eq!(3, nodeplane.nodes[0].acc);
    }

    #[test]
    fn negate() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(-42)));
        node_1_instructions[1] = Some(Instruction::Neg);
        node_1_instructions[2] = Some(Instruction::Neg);
        nodeplane.step();
        nodeplane.step();
        assert_eq!(42, nodeplane.nodes[0].acc);
        nodeplane.step();
        assert_eq!(-42, nodeplane.nodes[0].acc);
    }

    #[test]
    fn negate_zero() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Neg);
        nodeplane.step();
        assert_eq!(0, nodeplane.nodes[0].acc);
    }

    #[test]
    fn basic_sav() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node_1_instructions[1] = Some(Instruction::Sav);
        nodeplane.step();
        nodeplane.step();
        assert_eq!(42, nodeplane.nodes[0].bak);
    }

    #[test]
    fn basic_swp() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Add(Src::Literal(42)));
        node_1_instructions[1] = Some(Instruction::Sav);
        node_1_instructions[2] = Some(Instruction::Mov(
            Src::Literal(13),
            Dst::Register(Register::Acc),
        ));
        node_1_instructions[3] = Some(Instruction::Swp);
        nodeplane.step();
        nodeplane.step();
        nodeplane.step();
        nodeplane.step();
        assert_eq!(13, nodeplane.nodes[0].bak);
        assert_eq!(42, nodeplane.nodes[0].acc);
    }

    #[test]
    fn basic_port_mov() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Mov(
            Src::Literal(42),
            Dst::Port(Port::True(TruePort::Right)),
        ));
        node_1_instructions[1] = Some(Instruction::Mov(
            Src::Port(Port::True(TruePort::Right)),
            Dst::Register(Register::Acc),
        ));
        let node_2_instructions = nodeplane.get_node_instructions_mut(1);
        node_2_instructions[0] = Some(Instruction::Mov(
            Src::Port(Port::True(TruePort::Left)),
            Dst::Register(Register::Acc),
        ));
        node_2_instructions[1] = Some(Instruction::Mov(
            Src::Literal(13),
            Dst::Port(Port::True(TruePort::Left)),
        ));
        nodeplane.step();
        assert_eq!(Mode::Write, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Read, nodeplane.nodes[1].mode);
        assert_eq!(0, nodeplane.nodes[1].acc);
        assert_eq!(TruePort::Right, nodeplane.nodes[0].last_port.unwrap());
        assert_eq!(TruePort::Left, nodeplane.nodes[1].last_port.unwrap());
        nodeplane.step();
        assert_eq!(Mode::Run, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Run, nodeplane.nodes[1].mode);
        assert_eq!(42, nodeplane.nodes[1].acc);
        nodeplane.step();
        assert_eq!(Mode::Read, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Write, nodeplane.nodes[1].mode);
        nodeplane.step();
        assert_eq!(Mode::Run, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Run, nodeplane.nodes[1].mode);
        assert_eq!(13, nodeplane.nodes[0].acc);
    }

    #[test]
    fn nop_then_port_mov() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Mov(
            Src::Literal(42),
            Dst::Port(Port::True(TruePort::Right)),
        ));
        let node_2_instructions = nodeplane.get_node_instructions_mut(1);
        node_2_instructions[0] = Some(Instruction::Add(Src::Register(Register::Acc)));
        node_2_instructions[1] = Some(Instruction::Mov(
            Src::Port(Port::True(TruePort::Left)),
            Dst::Register(Register::Acc),
        ));
        nodeplane.step();
        assert_eq!(Mode::Write, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Run, nodeplane.nodes[1].mode);
        nodeplane.step();
        // note that the Read was instant
        assert_eq!(Mode::Run, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Run, nodeplane.nodes[1].mode);
        assert_eq!(42, nodeplane.nodes[1].acc);
    }

    #[test]
    fn port_mov_back() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Mov(
            Src::Literal(13),
            Dst::Port(Port::True(TruePort::Right)),
        ));
        node_1_instructions[1] = Some(Instruction::Mov(
            Src::Port(Port::True(TruePort::Right)),
            Dst::Register(Register::Acc),
        ));

        let node_2_instructions = nodeplane.get_node_instructions_mut(1);
        node_2_instructions[0] = Some(Instruction::Mov(
                Src::Port(Port::True(TruePort::Left)),
                Dst::Port(Port::True(TruePort::Left))
        ));

        nodeplane.step();
        assert_eq!(Mode::Write, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Read, nodeplane.nodes[1].mode);
        nodeplane.step();
        assert_eq!(Mode::Run, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Write, nodeplane.nodes[1].mode);
        nodeplane.step();
        assert_eq!(Mode::Run, nodeplane.nodes[0].mode);
        assert_eq!(Mode::Run, nodeplane.nodes[1].mode);
        assert_eq!(13, nodeplane.nodes[0].acc);
    }
}
