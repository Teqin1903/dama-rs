//! Watching a search without changing it (docs/web-api.md, Trace).
//!
//! The search reports what it does to a `Tracer`: the iterations, every node it enters and how it
//! leaves it, the table probes, the leaves and the repetitions. `NoTrace` ignores everything; its
//! `ON` is false, and the search calls a tracer only behind `if T::ON`, so with `NoTrace` the calls
//! and the work for their arguments are compiled away. `Recorder` keeps its own tables: the
//! statistics, each completed iteration with its root moves and principal variations, and, when
//! asked, the tree of one iteration. A tracer only reads what the search shows it; it never writes
//! to the transposition table, the move-ordering heuristics or the node counter.
//!
//! Values and windows are those of the search: from the root side's view (the search is minimax,
//! not negamax), so nodes with the root side to move maximise and the others minimise.

use crate::movegen::Move;

/// What a value says about a node's true value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueBound {
    Exact,
    /// The true value is at least this.
    Lower,
    /// The true value is at most this.
    Upper,
}

impl ValueBound {
    pub fn key(self) -> &'static str {
        match self {
            ValueBound::Exact => "exact",
            ValueBound::Lower => "lower",
            ValueBound::Upper => "upper",
        }
    }
}

/// Why a move was tried where it was in its node's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The transposition table's best move for the node.
    Tt,
    /// At the root, the previous iteration's best move.
    Pv,
    Killer,
    History,
    Promotion,
    /// A capture, in legal order among the captures.
    Capture,
    /// Legal order.
    None,
}

impl Reason {
    /// The name docs/web-api.md uses.
    pub fn key(self) -> &'static str {
        match self {
            Reason::Tt => "tt",
            Reason::Pv => "pv",
            Reason::Killer => "killer",
            Reason::History => "history",
            Reason::Promotion => "promotion",
            Reason::Capture => "capture",
            Reason::None => "none",
        }
    }
}

/// How a node is searched, seen from its parent. With principal variation search a move can enter
/// twice under its parent: first a `Scout`, then a `Research`, which replaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// With the parent's window.
    Full,
    /// With an empty window, to see whether the move can improve the parent.
    Scout,
    /// With the parent's window, after a `Scout` that landed inside it.
    Research,
}

impl Pass {
    /// The name docs/web-api.md uses.
    pub fn key(self) -> &'static str {
        match self {
            Pass::Full => "full",
            Pass::Scout => "scout",
            Pass::Research => "research",
        }
    }
}

/// What a probe of the transposition table found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtProbe {
    /// No entry for the position.
    Miss,
    /// An entry whose move is searched first, but whose value does not settle the node.
    Hit,
    /// An entry that settles the node: it is not searched.
    Cut,
}

impl TtProbe {
    pub fn key(self) -> &'static str {
        match self {
            TtProbe::Miss => "miss",
            TtProbe::Hit => "hit",
            TtProbe::Cut => "cut",
        }
    }
}

/// A node the search is about to enter.
#[derive(Clone, Copy, Debug)]
pub struct Enter<'m> {
    /// The move that leads to it, `None` at the root.
    pub mv: Option<&'m Move>,
    /// The index of `mv` in its parent's legal moves (0 at the root).
    pub index: usize,
    pub ply: u32,
    /// Plies left to the depth limit, 0 beyond it.
    pub draft: u32,
    /// Beyond the depth limit, where only captures are searched.
    pub quiescence: bool,
    /// The window `(alpha, beta)` the node is searched with, from the root side's view.
    pub window: (f64, f64),
    pub reason: Reason,
    pub pass: Pass,
}

/// What a search reports. Every method does nothing unless a tracer overrides it.
pub trait Tracer {
    /// Whether the search should report anything: false only for `NoTrace`.
    const ON: bool;
    /// An iteration of `depth` plies starts (a fixed-depth search is one iteration).
    fn iteration_start(&mut self, _depth: u32) {}
    /// The iteration has ended, completed or interrupted by the budget; `nodes` is the search's
    /// node count so far, every iteration together. An interrupted iteration leaves nodes that were
    /// entered and never left.
    fn iteration_end(&mut self, _depth: u32, _completed: bool, _nodes: u64) {}
    /// A try of the iteration in the aspiration window `window` failed with the best value `value`,
    /// and it starts again from the root; `nodes` as in `iteration_end`. The try's root has been
    /// left. Comes between `iteration_start` and `iteration_end`.
    fn retry(&mut self, _depth: u32, _window: (f64, f64), _value: f64, _nodes: u64) {}
    /// The search enters a node (it has already counted it).
    fn enter(&mut self, _node: &Enter) {}
    /// The node's side to move has `moves` legal moves.
    fn legal(&mut self, _moves: usize) {}
    fn tt(&mut self, _probe: TtProbe) {}
    /// The node is scored with the static evaluation `value`.
    fn leaf(&mut self, _value: f64) {}
    /// The node repeats a position of the game or of the path: a draw.
    fn repetition(&mut self) {}
    /// The search leaves the node with `value`, which is `bound` of
    /// its true value; `cutoff` when its last child caused a cutoff.
    fn exit(&mut self, _value: f64, _bound: ValueBound, _cutoff: bool) {}
}

/// The tracer of the arena: nothing is reported, and nothing is compiled in.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTrace;

impl Tracer for NoTrace {
    const ON: bool = false;
}

/// A tracer lent to the search.
impl<R: Tracer> Tracer for &mut R {
    const ON: bool = R::ON;
    fn iteration_start(&mut self, depth: u32) {
        (**self).iteration_start(depth);
    }
    fn iteration_end(&mut self, depth: u32, completed: bool, nodes: u64) {
        (**self).iteration_end(depth, completed, nodes);
    }
    fn retry(&mut self, depth: u32, window: (f64, f64), value: f64, nodes: u64) {
        (**self).retry(depth, window, value, nodes);
    }
    fn enter(&mut self, node: &Enter) {
        (**self).enter(node);
    }
    fn legal(&mut self, moves: usize) {
        (**self).legal(moves);
    }
    fn tt(&mut self, probe: TtProbe) {
        (**self).tt(probe);
    }
    fn leaf(&mut self, value: f64) {
        (**self).leaf(value);
    }
    fn repetition(&mut self) {
        (**self).repetition();
    }
    fn exit(&mut self, value: f64, bound: ValueBound, cutoff: bool) {
        (**self).exit(value, bound, cutoff);
    }
}

/// Counts over a whole search, every iteration together (the interrupted one included).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub tt_probes: u64,
    pub tt_hits: u64,
    pub tt_cutoffs: u64,
    /// Nodes left early because a child's value reached the other end of the window.
    pub beta_cutoffs: u64,
    /// The part of `beta_cutoffs` caused by the first child tried.
    pub first_move_cutoffs: u64,
    pub repetition_draws: u64,
}

/// A root move as one iteration left it.
#[derive(Clone, Debug, PartialEq)]
pub struct RootMove {
    pub mv: Move,
    /// Its index in the legal moves.
    pub index: usize,
    pub value: f64,
    pub bound: ValueBound,
    /// 1 for the first move tried.
    pub order: u32,
    pub reason: Reason,
    /// Nodes in its subtree, itself included, its test's too.
    pub nodes: u64,
    /// Its principal variation, the move itself first.
    pub pv: Vec<Move>,
}

/// A try of an iteration whose aspiration window failed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attempt {
    pub window: (f64, f64),
    /// The best value it found: at or below the window's lower end, or just above it, or at or
    /// above its upper end.
    pub value: f64,
    /// The nodes the search had counted when it failed, earlier iterations included.
    pub nodes: u64,
}

/// A completed iteration.
#[derive(Clone, Debug, PartialEq)]
pub struct Iteration {
    pub depth: u32,
    /// The nodes the search had counted when this iteration completed, earlier iterations
    /// included.
    pub nodes: u64,
    pub ms: f64,
    /// The root moves in the order they were tried, in the try that counted.
    pub root: Vec<RootMove>,
    /// The tries before that one, whose aspiration windows failed.
    pub attempts: Vec<Attempt>,
}

/// Which iteration's tree to record, and how many of its nodes at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceRequest {
    /// The depth of the iteration, or -1 for the last completed one.
    pub iteration: i32,
    pub max_nodes: usize,
}

/// A recorded node (docs/web-api.md, Trace).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceNode {
    pub parent: Option<usize>,
    pub mv: Option<Move>,
    pub ply: u32,
    pub draft: u32,
    pub quiescence: bool,
    pub window: (f64, f64),
    pub value: f64,
    pub bound: ValueBound,
    /// The static evaluation, when the node returned it.
    pub static_value: Option<f64>,
    /// In the order tried; only the recorded ones.
    pub children: Vec<usize>,
    pub legal: usize,
    /// Why this node's move was tried where it was; `None` at the root.
    pub order: Option<Reason>,
    pub pass: Pass,
    pub cutoff: bool,
    pub tt: Option<TtProbe>,
    pub repetition: bool,
    /// Nodes in its subtree, itself included, recorded or not.
    pub sub: u64,
    /// When it was entered and left, counting every entry and exit of the iteration.
    pub visit: u64,
    pub exit: u64,
}

/// The tree of one iteration.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub iteration: u32,
    /// Every node the try that counted entered, the root included.
    pub nodes_total: u64,
    /// Whether `max_nodes` stopped the recording before the iteration ended.
    pub truncated: bool,
    /// Node `i` is the `i`-th node entered.
    pub nodes: Vec<TraceNode>,
}

/// A node on the path from the root, while the search is inside it.
#[derive(Clone, Debug)]
struct Frame {
    /// Its index in the tree, if recorded.
    id: Option<usize>,
    mv: Option<Move>,
    index: usize,
    ply: u32,
    window: (f64, f64),
    reason: Reason,
    pass: Pass,
    /// Nodes entered in the iteration before it.
    entered_before: u64,
    /// The nodes of the test it searches again.
    test_nodes: u64,
    children: u32,
    /// The best child value so far, and its principal variation (the child's move first).
    best: Option<f64>,
    pv: Vec<Move>,
    /// `best` and `pv` before the last child if it was a test: a research replaces the test.
    before_test: Option<(Option<f64>, Vec<Move>)>,
}

/// The iteration being searched.
#[derive(Clone, Debug)]
struct Current {
    depth: u32,
    start_ms: f64,
    stack: Vec<Frame>,
    root: Vec<RootMove>,
    entered: u64,
    /// Entries and exits so far.
    events: u64,
    tree: Vec<TraceNode>,
    truncated: bool,
    attempts: Vec<Attempt>,
}

/// The tracer of `think` and `analyze`: statistics, iterations and, if asked, one tree.
#[derive(Clone, Debug)]
pub struct Recorder {
    clock: fn() -> f64,
    request: Option<TraceRequest>,
    pub stats: Stats,
    /// The completed iterations, in order.
    pub iterations: Vec<Iteration>,
    /// The tree asked for, once its iteration is completed.
    pub tree: Option<Tree>,
    current: Option<Current>,
}

impl Recorder {
    /// A recorder that reads the time in milliseconds from `clock` and records the tree of
    /// `request`'s iteration if there is a request.
    pub fn new(clock: fn() -> f64, request: Option<TraceRequest>) -> Recorder {
        Recorder {
            clock,
            request,
            stats: Stats::default(),
            iterations: Vec::new(),
            tree: None,
            current: None,
        }
    }

    /// Whether a tree was asked for.
    pub fn requested(&self) -> bool {
        self.request.is_some()
    }
}

impl Tracer for Recorder {
    const ON: bool = true;

    fn iteration_start(&mut self, depth: u32) {
        self.current = Some(Current {
            depth,
            start_ms: (self.clock)(),
            stack: Vec::new(),
            root: Vec::new(),
            entered: 0,
            events: 0,
            tree: Vec::new(),
            truncated: false,
            attempts: Vec::new(),
        });
    }

    fn retry(&mut self, depth: u32, window: (f64, f64), value: f64, nodes: u64) {
        let current = self.current.as_mut().expect("an iteration was started");
        debug_assert_eq!(current.depth, depth);
        debug_assert!(current.stack.is_empty(), "the try's root has been left");
        current.attempts.push(Attempt {
            window,
            value,
            nodes,
        });
        // Only the try that counts keeps its root moves and its tree.
        current.root.clear();
        current.entered = 0;
        current.events = 0;
        current.tree.clear();
        current.truncated = false;
    }

    fn iteration_end(&mut self, depth: u32, completed: bool, nodes: u64) {
        let current = self.current.take().expect("an iteration was started");
        debug_assert_eq!(current.depth, depth);
        if !completed {
            return;
        }
        if let Some(request) = self.request {
            let wanted = request.iteration < 0 || request.iteration as u32 == depth;
            if wanted {
                self.tree = Some(Tree {
                    iteration: depth,
                    nodes_total: current.entered,
                    truncated: current.truncated,
                    nodes: current.tree,
                });
            }
        }
        self.iterations.push(Iteration {
            depth,
            nodes,
            ms: (self.clock)() - current.start_ms,
            root: current.root,
            attempts: current.attempts,
        });
    }

    fn enter(&mut self, node: &Enter) {
        let records = self.requested();
        let max_nodes = self.request.map_or(0, |r| r.max_nodes);
        let current = self.current.as_mut().expect("an iteration was started");
        // A research replaces its test in what the parent knows; the tree shows both.
        let mut test_nodes = 0;
        if node.pass == Pass::Research {
            let parent = current.stack.last_mut().expect("a research has a parent");
            let (best, pv) = parent.before_test.take().expect("a test came before");
            (parent.best, parent.pv) = (best, pv);
            parent.children -= 1;
            if parent.ply == 0 {
                test_nodes = current.root.pop().expect("a test came before").nodes;
            }
        }
        let parent = current.stack.last().and_then(|f| f.id);
        let parent_recorded = current.stack.is_empty() || parent.is_some();
        let id = if records && parent_recorded && current.tree.len() < max_nodes {
            let id = current.tree.len();
            if let Some(p) = parent {
                current.tree[p].children.push(id);
            }
            current.tree.push(TraceNode {
                parent,
                mv: node.mv.cloned(),
                ply: node.ply,
                draft: node.draft,
                quiescence: node.quiescence,
                window: node.window,
                value: 0.0,
                bound: ValueBound::Exact,
                static_value: None,
                children: Vec::new(),
                legal: 0,
                order: node.mv.map(|_| node.reason),
                pass: node.pass,
                cutoff: false,
                tt: None,
                repetition: false,
                sub: 0,
                visit: current.events,
                exit: 0,
            });
            Some(id)
        } else {
            current.truncated |= records;
            None
        };
        current.events += 1;
        current.stack.push(Frame {
            id,
            mv: node.mv.cloned(),
            index: node.index,
            ply: node.ply,
            window: node.window,
            reason: node.reason,
            pass: node.pass,
            entered_before: current.entered,
            test_nodes,
            children: 0,
            best: None,
            pv: Vec::new(),
            before_test: None,
        });
        current.entered += 1;
    }

    fn legal(&mut self, moves: usize) {
        if let Some(node) = self.top_node() {
            node.legal = moves;
        }
    }

    fn tt(&mut self, probe: TtProbe) {
        self.stats.tt_probes += 1;
        match probe {
            TtProbe::Miss => {}
            TtProbe::Hit => self.stats.tt_hits += 1,
            TtProbe::Cut => {
                self.stats.tt_hits += 1;
                self.stats.tt_cutoffs += 1;
            }
        }
        if let Some(node) = self.top_node() {
            node.tt = Some(probe);
        }
    }

    fn leaf(&mut self, value: f64) {
        if let Some(node) = self.top_node() {
            node.static_value = Some(value);
        }
    }

    fn repetition(&mut self) {
        self.stats.repetition_draws += 1;
        if let Some(node) = self.top_node() {
            node.repetition = true;
        }
    }

    fn exit(&mut self, value: f64, bound: ValueBound, cutoff: bool) {
        let current = self.current.as_mut().expect("an iteration was started");
        let frame = current.stack.pop().expect("every exit has its entry");
        if cutoff {
            self.stats.beta_cutoffs += 1;
            if frame.children == 1 {
                self.stats.first_move_cutoffs += 1;
            }
        }
        let sub = current.entered - frame.entered_before;
        if let Some(id) = frame.id {
            let node = &mut current.tree[id];
            node.value = value;
            node.bound = bound;
            node.cutoff = cutoff;
            node.sub = sub;
            node.exit = current.events;
        }
        current.events += 1;
        let Some(parent) = current.stack.last_mut() else {
            return;
        };
        parent.children += 1;
        // The root side maximises at even plies, as the search does; the first child always
        // improves, and later ones only when strictly better (the search's best move).
        let maximising = parent.ply % 2 == 0;
        let improves = match parent.best {
            None => true,
            Some(best) => (maximising && value > best) || (!maximising && value < best),
        };
        let mv = frame.mv.expect("a child has a move");
        let mut pv = Vec::with_capacity(frame.pv.len() + 1);
        pv.push(mv.clone());
        pv.extend(frame.pv);
        if parent.ply == 0 {
            // As the root decides: exact above the window's lower end, an upper bound at or below
            // it, even where the child's value is exact for itself (a leaf, a win).
            let root_bound = if value > frame.window.0 {
                bound
            } else {
                ValueBound::Upper
            };
            current.root.push(RootMove {
                mv,
                index: frame.index,
                value,
                bound: root_bound,
                order: parent.children,
                reason: frame.reason,
                nodes: sub + frame.test_nodes,
                pv: pv.clone(),
            });
        }
        if frame.pass == Pass::Scout {
            parent.before_test = Some((parent.best, parent.pv.clone()));
        }
        if improves {
            parent.best = Some(value);
            parent.pv = pv;
        }
    }
}

impl Recorder {
    /// The recorded node the search is in, if it is recorded.
    fn top_node(&mut self) -> Option<&mut TraceNode> {
        let current = self.current.as_mut()?;
        let id = current.stack.last()?.id?;
        Some(&mut current.tree[id])
    }
}
