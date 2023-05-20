#[derive(Debug, Copy, Clone)]
enum Register {
    Acc,
    Nil,
    // There is also a BAK register but it is not addressable
}

#[derive(Debug, Copy, Clone)]
enum TruePort {
    Up,
    Down,
    Left,
    Right,
    // originally a pseudoport, but oh well
    Any,
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
        self.current_instruction = instructions[self.instruction_pointer as usize];
    }
    fn read_step(&mut self) {
        if self.mode == Mode::Read || self.mode == Mode::Write {
            return;
        }
        if let Some(instruction) = self.current_instruction {
            if let Some(src) = instruction.get_read_src() {
                match src {
                    Src::Port(port) => {
                        self.mode = Mode::Read;
                        self.direction = Some(self.map_port(port));
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
            None => self.instruction_pointer = 0,
            _ => unimplemented!(),
        };
        if self.mode == Mode::Run {
            self.instruction_pointer += 1;
            if self.instruction_pointer >= INSTRUCTIONS_PER_NODE as u8 {
                self.instruction_pointer = 0;
            }
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
                }
            }
            Dst::Register(register) => {
                match register {
                    Register::Acc => self.acc = value.unwrap(),
                    Register::Nil => (),
                }
            }
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
}

static PORT_LUT: [(u8, u8, u8, u8); NODES_PER_PLANE] = [
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

trait Plane {
    fn step(&mut self) {}
}

const NODES_PER_PLANE: usize = 12;
const INSTRUCTIONS_PER_NODE: usize = 21;

struct ExecutionPlane {
    nodes: [ExecutionNode; NODES_PER_PLANE],
    ports: [Option<i16>; 31],
    queued_writes: [Option<i16>; 31],
    instructions: Box<[Option<Instruction>; NODES_PER_PLANE * INSTRUCTIONS_PER_NODE]>,
}

impl ExecutionPlane {
    fn new() -> Self {
        const NODE: ExecutionNode = ExecutionNode::new();
        Self {
            nodes: [NODE; NODES_PER_PLANE],
            ports: [None; 31],
            queued_writes: [None; 31],
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
            node.read_step();
            if node.mode == Mode::Read {
                if let Some(direction) = node.direction {
                    let mut port = self.ports[map_port(direction, i)];
                    node.port_read_buffer = port.take();
                }
            }
            node.step();
            if node.mode == Mode::Write {
                if let Some(direction) = node.direction {
                    if node.port_write_buffer.is_some() {
                        let index = map_port(direction, i);
                        self.queued_writes[index] = node.port_write_buffer.take();
                    }
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
        }
    }
}

fn main() {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn foo() {
        let mut nodeplane = ExecutionPlane::new();
        let node_1_instructions = nodeplane.get_node_instructions_mut(0);
        node_1_instructions[0] = Some(Instruction::Swp);
        let node_12_instructions = nodeplane.get_node_instructions_mut(11);
        node_12_instructions[0] = Some(Instruction::Sav);
        nodeplane.step();
    }

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
        node_1_instructions[2] = Some(Instruction::Mov(Src::Literal(13), Dst::Register(Register::Acc)));
        node_1_instructions[3] = Some(Instruction::Swp);
        nodeplane.step();
        nodeplane.step();
        nodeplane.step();
        nodeplane.step();
        assert_eq!(13, nodeplane.nodes[0].bak);
        assert_eq!(42, nodeplane.nodes[0].acc);
    }

    /*
    #[test]
    fn basic_port_mov() {
        let mut node1 = Node::new();
        let mut node2 = Node::new();
        let channels = vec![Channel::new(None)];
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
        let channels = vec![Channel::new(None)];
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
        let channels = vec![Channel::new(None), Channel::new(None)];
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
    */
}
