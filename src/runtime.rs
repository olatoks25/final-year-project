use crate::dsl::{Expr, Program, Statement};
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(BlockReason),
    Sleeping(u64),
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockReason {
    Send { channel: String, expr: Expr },
    Recv { channel: String, var: String },
}

#[derive(Debug, Clone)]
struct Task {
    name: String,
    priority: u8,
    pc: usize,
    state: TaskState,
    statements: Vec<Statement>,
    locals: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct Channel {
    capacity: usize,
    queue: VecDeque<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub trace: Vec<String>,
    pub finished: bool,
    pub ticks_executed: u64,
}

pub struct Simulator {
    tasks: Vec<Task>,
    channels: HashMap<String, Channel>,
    ready: BTreeMap<u8, VecDeque<usize>>,
    current_tick: u64,
    trace: Vec<String>,
}

impl Simulator {
    pub fn new(program: Program) -> Result<Self, String> {
        let mut channels = HashMap::new();
        for ch in program.channels {
            if channels
                .insert(
                    ch.name.clone(),
                    Channel {
                        capacity: ch.capacity,
                        queue: VecDeque::new(),
                    },
                )
                .is_some()
            {
                return Err(format!("duplicate channel '{}'", ch.name));
            }
        }

        let mut task_defs = HashMap::new();
        for task in program.tasks {
            if task_defs.insert(task.name.clone(), task).is_some() {
                return Err("duplicate task declaration".to_string());
            }
        }

        let mut tasks = Vec::new();
        for name in &program.run_order {
            let Some(task) = task_defs.get(name) else {
                return Err(format!("run references undefined task '{name}'"));
            };
            tasks.push(Task {
                name: task.name.clone(),
                priority: task.priority,
                pc: 0,
                state: TaskState::Ready,
                statements: task.statements.clone(),
                locals: HashMap::new(),
            });
        }

        let mut sim = Self {
            tasks,
            channels,
            ready: BTreeMap::new(),
            current_tick: 0,
            trace: Vec::new(),
        };

        for i in 0..sim.tasks.len() {
            sim.enqueue_ready(i);
        }

        Ok(sim)
    }

    fn enqueue_ready(&mut self, id: usize) {
        self.tasks[id].state = TaskState::Ready;
        self.ready
            .entry(self.tasks[id].priority)
            .or_default()
            .push_back(id);
    }

    fn pop_next_ready(&mut self) -> Option<usize> {
        for (_, queue) in self.ready.iter_mut().rev() {
            if let Some(id) = queue.pop_front() {
                return Some(id);
            }
        }
        None
    }

    fn is_all_finished(&self) -> bool {
        self.tasks.iter().all(|t| t.state == TaskState::Finished)
    }

    fn has_sleepers(&self) -> bool {
        self.tasks
            .iter()
            .any(|t| matches!(t.state, TaskState::Sleeping(_)))
    }

    fn tick_prefix(&self) -> String {
        format!("tick {:03}", self.current_tick)
    }

    fn eval_expr_for_task(&self, task_id: usize, expr: &Expr) -> String {
        match expr {
            Expr::StringLiteral(v) => v.clone(),
            Expr::Variable(name) => self.tasks[task_id]
                .locals
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("<undefined:{name}>")),
        }
    }

    fn wake_sleepers(&mut self) {
        let mut to_ready = Vec::new();
        for (id, task) in self.tasks.iter().enumerate() {
            if let TaskState::Sleeping(wake_tick) = task.state {
                if wake_tick <= self.current_tick {
                    to_ready.push(id);
                }
            }
        }
        for id in to_ready {
            self.trace.push(format!(
                "{} [{}] wake",
                self.tick_prefix(),
                self.tasks[id].name
            ));
            self.enqueue_ready(id);
        }
    }

    fn try_unblock_tasks(&mut self) {
        for id in 0..self.tasks.len() {
            let reason = match self.tasks[id].state.clone() {
                TaskState::Blocked(reason) => reason,
                _ => continue,
            };

            match reason {
                BlockReason::Send { channel, expr } => {
                    let can_send = self
                        .channels
                        .get(&channel)
                        .map(|c| c.queue.len() < c.capacity)
                        .unwrap_or(false);
                    if can_send {
                        let value = self.eval_expr_for_task(id, &expr);
                        self.channels
                            .get_mut(&channel)
                            .expect("channel exists while blocked")
                            .queue
                            .push_back(value.clone());
                        self.trace.push(format!(
                            "{} [{}] send {} => {} (unblocked)",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel,
                            value
                        ));
                        self.tasks[id].pc += 1;
                        if self.tasks[id].pc >= self.tasks[id].statements.len() {
                            self.tasks[id].state = TaskState::Finished;
                            self.trace.push(format!(
                                "{} [{}] finished",
                                self.tick_prefix(),
                                self.tasks[id].name
                            ));
                        } else {
                            self.enqueue_ready(id);
                        }
                    }
                }
                BlockReason::Recv { channel, var } => {
                    let value = if let Some(ch) = self.channels.get_mut(&channel) {
                        ch.queue.pop_front()
                    } else {
                        None
                    };
                    if let Some(value) = value {
                        self.tasks[id].locals.insert(var.clone(), value.clone());
                        self.trace.push(format!(
                            "{} [{}] recv {} <= {} (unblocked)",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel,
                            value
                        ));
                        self.tasks[id].pc += 1;
                        if self.tasks[id].pc >= self.tasks[id].statements.len() {
                            self.tasks[id].state = TaskState::Finished;
                            self.trace.push(format!(
                                "{} [{}] finished",
                                self.tick_prefix(),
                                self.tasks[id].name
                            ));
                        } else {
                            self.enqueue_ready(id);
                        }
                    }
                }
            }
        }
    }

    fn execute_one_statement(&mut self, id: usize) {
        self.tasks[id].state = TaskState::Running;

        if self.tasks[id].pc >= self.tasks[id].statements.len() {
            self.tasks[id].state = TaskState::Finished;
            self.trace.push(format!(
                "{} [{}] finished",
                self.tick_prefix(),
                self.tasks[id].name
            ));
            return;
        }

        let statement = self.tasks[id].statements[self.tasks[id].pc].clone();
        match statement {
            Statement::Print(expr) => {
                let value = self.eval_expr_for_task(id, &expr);
                self.trace.push(format!(
                    "{} [{}] print {}",
                    self.tick_prefix(),
                    self.tasks[id].name,
                    value
                ));
                self.tasks[id].pc += 1;
                if self.tasks[id].pc >= self.tasks[id].statements.len() {
                    self.tasks[id].state = TaskState::Finished;
                    self.trace.push(format!(
                        "{} [{}] finished",
                        self.tick_prefix(),
                        self.tasks[id].name
                    ));
                } else {
                    self.enqueue_ready(id);
                }
            }
            Statement::Yield => {
                self.trace.push(format!(
                    "{} [{}] yield",
                    self.tick_prefix(),
                    self.tasks[id].name
                ));
                self.tasks[id].pc += 1;
                if self.tasks[id].pc >= self.tasks[id].statements.len() {
                    self.tasks[id].state = TaskState::Finished;
                    self.trace.push(format!(
                        "{} [{}] finished",
                        self.tick_prefix(),
                        self.tasks[id].name
                    ));
                } else {
                    self.enqueue_ready(id);
                }
            }
            Statement::Sleep(duration) => {
                let wake_tick = self.current_tick + duration.max(1);
                self.trace.push(format!(
                    "{} [{}] sleep {}",
                    self.tick_prefix(),
                    self.tasks[id].name,
                    duration
                ));
                self.tasks[id].pc += 1;
                self.tasks[id].state = TaskState::Sleeping(wake_tick);
            }
            Statement::Send { channel, expr } => {
                let value = self.eval_expr_for_task(id, &expr);
                if let Some(ch) = self.channels.get_mut(&channel) {
                    if ch.queue.len() < ch.capacity {
                        ch.queue.push_back(value.clone());
                        self.trace.push(format!(
                            "{} [{}] send {} => {}",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel,
                            value
                        ));
                        self.tasks[id].pc += 1;
                        if self.tasks[id].pc >= self.tasks[id].statements.len() {
                            self.tasks[id].state = TaskState::Finished;
                            self.trace.push(format!(
                                "{} [{}] finished",
                                self.tick_prefix(),
                                self.tasks[id].name
                            ));
                        } else {
                            self.enqueue_ready(id);
                        }
                    } else {
                        self.tasks[id].state = TaskState::Blocked(BlockReason::Send {
                            channel: channel.clone(),
                            expr,
                        });
                        self.trace.push(format!(
                            "{} [{}] blocked send {} (queue full)",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel
                        ));
                    }
                } else {
                    self.trace.push(format!(
                        "{} [{}] runtime error: unknown channel {}",
                        self.tick_prefix(),
                        self.tasks[id].name,
                        channel
                    ));
                    self.tasks[id].state = TaskState::Finished;
                }
            }
            Statement::Recv { channel, var } => {
                if let Some(ch) = self.channels.get_mut(&channel) {
                    if let Some(value) = ch.queue.pop_front() {
                        self.tasks[id].locals.insert(var.clone(), value.clone());
                        self.trace.push(format!(
                            "{} [{}] recv {} <= {}",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel,
                            value
                        ));
                        self.tasks[id].pc += 1;
                        if self.tasks[id].pc >= self.tasks[id].statements.len() {
                            self.tasks[id].state = TaskState::Finished;
                            self.trace.push(format!(
                                "{} [{}] finished",
                                self.tick_prefix(),
                                self.tasks[id].name
                            ));
                        } else {
                            self.enqueue_ready(id);
                        }
                    } else {
                        self.tasks[id].state = TaskState::Blocked(BlockReason::Recv {
                            channel: channel.clone(),
                            var,
                        });
                        self.trace.push(format!(
                            "{} [{}] blocked recv {} (queue empty)",
                            self.tick_prefix(),
                            self.tasks[id].name,
                            channel
                        ));
                    }
                } else {
                    self.trace.push(format!(
                        "{} [{}] runtime error: unknown channel {}",
                        self.tick_prefix(),
                        self.tasks[id].name,
                        channel
                    ));
                    self.tasks[id].state = TaskState::Finished;
                }
            }
        }
    }

    pub fn run(&mut self, max_ticks: u64) -> RunResult {
        let mut ticks = 0;

        while ticks < max_ticks {
            self.current_tick = ticks;
            self.wake_sleepers();
            self.try_unblock_tasks();

            if self.is_all_finished() {
                return RunResult {
                    trace: self.trace.clone(),
                    finished: true,
                    ticks_executed: ticks,
                };
            }

            if let Some(task_id) = self.pop_next_ready() {
                self.execute_one_statement(task_id);
            } else {
                self.trace
                    .push(format!("{} [idle] idle", self.tick_prefix()));
                if !self.has_sleepers() {
                    self.trace.push(format!(
                        "{} [kernel] deadlock: no runnable tasks",
                        self.tick_prefix()
                    ));
                    return RunResult {
                        trace: self.trace.clone(),
                        finished: false,
                        ticks_executed: ticks + 1,
                    };
                }
            }

            ticks += 1;
        }

        RunResult {
            trace: self.trace.clone(),
            finished: self.is_all_finished(),
            ticks_executed: ticks,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dsl::parse_program;

    use super::*;

    fn run(src: &str, max_ticks: u64) -> RunResult {
        let program = parse_program(src).expect("valid source");
        let mut sim = Simulator::new(program).expect("valid runtime setup");
        sim.run(max_ticks)
    }

    #[test]
    fn priority_scheduling_picks_high_first() {
        let src = r#"
channel bus capacity 1;
task low priority 1 { print "low"; }
task high priority 2 { print "high"; }
run low, high;
"#;
        let result = run(src, 10);
        let first_print = result
            .trace
            .iter()
            .find(|line| line.contains("print"))
            .expect("print line");
        assert!(first_print.contains("high"));
    }

    #[test]
    fn fairness_round_robin_equal_priority() {
        let src = r#"
task a priority 1 { print "a1"; yield; print "a2"; }
task b priority 1 { print "b1"; yield; print "b2"; }
run a, b;
"#;
        let result = run(src, 20);
        let prints: Vec<_> = result
            .trace
            .iter()
            .filter(|l| l.contains("print"))
            .cloned()
            .collect();
        assert!(prints[0].contains("a1"));
        assert!(prints[1].contains("b1"));
    }

    #[test]
    fn sleep_wakes_task() {
        let src = r#"
task t priority 1 { print "start"; sleep 2; print "end"; }
run t;
"#;
        let result = run(src, 10);
        assert!(result.trace.iter().any(|l| l.contains("sleep 2")));
        assert!(result.trace.iter().any(|l| l.contains("wake")));
        assert!(result.trace.iter().any(|l| l.contains("print end")));
    }

    #[test]
    fn channel_send_recv_blocks_and_unblocks() {
        let src = r#"
channel q capacity 1;
task producer priority 2 { send q "m1"; send q "m2"; print "done"; }
task consumer priority 1 { recv q as m; print m; recv q as n; print n; }
run producer, consumer;
"#;
        let result = run(src, 30);
        assert!(result.trace.iter().any(|l| l.contains("blocked send")));
        assert!(result.trace.iter().any(|l| l.contains("recv q <= m1")));
        assert!(result.trace.iter().any(|l| l.contains("recv q <= m2")));
    }

    #[test]
    fn idle_tick_emitted_when_nothing_runnable() {
        let src = r#"
task t priority 1 { sleep 2; print "done"; }
run t;
"#;
        let result = run(src, 10);
        assert!(result.trace.iter().any(|l| l.contains("[idle] idle")));
    }
}
