//! The arena: the end-of-game rules of playGame in its order, the random opening plies and the
//! seeds of a match. Every test here goes through the public API, the way an agent written
//! outside the engine (an external program, later) would be played.

use engine::{
    Agent, AgentError, AgentSpec, Board, Color, EndReason, GameRecord, MatchSettings, Move,
    Mulberry32, play_game, play_match,
};

/// Plays a script of moves, in order, by their notation. A move that is not legal is an error, so
/// a broken script fails the test instead of testing something else.
struct Scripted {
    moves: Vec<String>,
    next: usize,
}

impl Scripted {
    fn new(moves: &[&str]) -> Scripted {
        Scripted {
            moves: moves.iter().map(|m| m.to_string()).collect(),
            next: 0,
        }
    }
}

impl Agent for Scripted {
    fn choose<'m>(
        &mut self,
        _board: &Board,
        _side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let want = self
            .moves
            .get(self.next)
            .ok_or_else(|| AgentError("the script has ended".to_string()))?;
        self.next += 1;
        legal
            .iter()
            .find(|mv| mv.to_string() == *want)
            .ok_or_else(|| AgentError(format!("{want} is not legal here")))
    }
}

/// Plays `legal[0]` and counts how often it was asked.
#[derive(Default)]
struct First {
    asked: u32,
}

impl Agent for First {
    fn choose<'m>(
        &mut self,
        _board: &Board,
        _side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        self.asked += 1;
        Ok(&legal[0])
    }
}

/// No random opening plies, and the draw at ply 400.
const NO_OPENING: MatchSettings = MatchSettings {
    opening_plies: 0,
    max_plies: 400,
};

fn play(
    board: &str,
    side: Color,
    white: &mut dyn Agent,
    black: &mut dyn Agent,
    settings: MatchSettings,
) -> GameRecord {
    let board: Board = board.parse().unwrap();
    play_game(
        board,
        side,
        white,
        black,
        &mut Mulberry32::new(1),
        &settings,
    )
    .unwrap()
}

fn notations(moves: &[Move]) -> Vec<String> {
    moves.iter().map(Move::to_string).collect()
}

#[test]
fn a_side_without_a_legal_move_loses() {
    // White's man on a1 is boxed in by a2/a3 and b1/c1: no step and no capture.
    let game = play(
        "......../......../......../......../......../b......./b......./wbb.....",
        Color::White,
        &mut First::default(),
        &mut First::default(),
        NO_OPENING,
    );
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (Some(Color::Black), EndReason::NoMove, 0)
    );
    // Black's man on a8 is boxed in the same way.
    let game = play(
        "bww...../w......./w......./......../......../......../......../........",
        Color::Black,
        &mut First::default(),
        &mut First::default(),
        NO_OPENING,
    );
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (Some(Color::White), EndReason::NoMove, 0)
    );
}

#[test]
fn one_piece_each_is_a_draw() {
    let game = play(
        "......../......../......../......../...W..../......../......../.......B",
        Color::White,
        &mut First::default(),
        &mut First::default(),
        NO_OPENING,
    );
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (None, EndReason::OnePieceEach, 0)
    );
}

/// White: king a2, man a1. Black: king h8, man h7. No capture is possible while White's king
/// stays in files a-f below rank 7 and Black's king stays on g8 or h8.
const KINGS: &str = ".......B/.......b/......../......../......../......../W......./w.......";

#[test]
fn the_third_occurrence_of_a_position_is_a_draw() {
    let mut white = Scripted::new(&["a2-b2", "b2-a2", "a2-b2", "b2-a2"]);
    let mut black = Scripted::new(&["h8-g8", "g8-h8", "h8-g8", "g8-h8"]);
    let game = play(KINGS, Color::White, &mut white, &mut black, NO_OPENING);
    // The start position occurs at plies 0, 4 and 8.
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (None, EndReason::Repetition, 8)
    );
}

/// White's king visits 25 new squares while Black's king goes back and forth: 50 plies without a
/// capture, and no position occurs twice.
fn quiet_tour() -> (Scripted, Scripted) {
    let tour = [
        "a2", "b2", "c2", "d2", "e2", "f2", "f3", "e3", "d3", "c3", "b3", "a3", "a4", "b4", "c4",
        "d4", "e4", "f4", "f5", "e5", "d5", "c5", "b5", "a5", "a6", "b6",
    ];
    let white: Vec<String> = tour
        .windows(2)
        .map(|w| format!("{}-{}", w[0], w[1]))
        .collect();
    let black: Vec<&str> = ["h8-g8", "g8-h8"].into_iter().cycle().take(25).collect();
    let white: Vec<&str> = white.iter().map(String::as_str).collect();
    (Scripted::new(&white), Scripted::new(&black))
}

#[test]
fn fifty_quiet_plies_are_a_draw_and_are_checked_before_the_cap() {
    let (mut white, mut black) = quiet_tour();
    let game = play(KINGS, Color::White, &mut white, &mut black, NO_OPENING);
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (None, EndReason::QuietPlies, 50)
    );

    // At ply 50 both rules apply; playGame checks the quiet plies first.
    let (mut white, mut black) = quiet_tour();
    let cap_at_50 = MatchSettings {
        opening_plies: 0,
        max_plies: 50,
    };
    let game = play(KINGS, Color::White, &mut white, &mut black, cap_at_50);
    assert_eq!((game.reason, game.plies()), (EndReason::QuietPlies, 50));

    // One ply earlier only the cap applies.
    let (mut white, mut black) = quiet_tour();
    let cap_at_49 = MatchSettings {
        opening_plies: 0,
        max_plies: 49,
    };
    let game = play(KINGS, Color::White, &mut white, &mut black, cap_at_49);
    assert_eq!((game.reason, game.plies()), (EndReason::Cap, 49));
}

#[test]
fn the_cap_ends_a_game() {
    let settings = MatchSettings {
        opening_plies: 0,
        max_plies: 3,
    };
    let start = Board::start().to_string();
    let game = play(
        &start,
        Color::White,
        &mut First::default(),
        &mut First::default(),
        settings,
    );
    assert_eq!(
        (game.winner, game.reason, game.plies()),
        (None, EndReason::Cap, 3)
    );
}

#[test]
fn opening_plies_are_drawn_by_the_arena_without_asking_the_agents() {
    let settings = MatchSettings {
        opening_plies: 2,
        max_plies: 4,
    };
    let (mut white, mut black) = (First::default(), First::default());
    let game = play_game(
        Board::start(),
        Color::White,
        &mut white,
        &mut black,
        &mut Mulberry32::new(2026),
        &settings,
    )
    .unwrap();

    // Plies 0 and 1: legal[floor(rnd() * len)] with the opening generator.
    let mut rng = Mulberry32::new(2026);
    let mut board = Board::start();
    let mut expected = Vec::new();
    for side in [Color::White, Color::Black] {
        let legal = board.legal_moves(side);
        let mv = legal[rng.next_index(legal.len())].clone();
        board = board.apply(&mv);
        expected.push(mv);
    }
    assert_eq!(game.moves[..2], expected[..]);
    // Plies 2 and 3 are the agents' moves, one each.
    assert_eq!((white.asked, black.asked), (1, 1));
    assert_eq!(game.plies(), 4);
}

fn spec(name: &str) -> AgentSpec {
    name.parse().unwrap()
}

/// A match between the agents named `a` and `b`.
fn match_games(a: &str, b: &str, games: u32, seed: u32) -> Vec<engine::MatchGame> {
    let (mut a, mut b) = (spec(a).build(), spec(b).build());
    play_match(
        a.as_mut(),
        b.as_mut(),
        games,
        seed,
        &MatchSettings::REFERENCE,
    )
    .unwrap()
    .games
}

#[test]
fn colours_alternate_and_the_same_seed_gives_the_same_games() {
    let games = match_games("tuned", "random", 4, 7);
    let a_white: Vec<bool> = games.iter().map(|g| g.a_white).collect();
    assert_eq!(a_white, [true, false, true, false]);
    assert_eq!(games, match_games("tuned", "random", 4, 7));
    assert_ne!(games, match_games("tuned", "random", 4, 8));
}

#[test]
fn a_game_does_not_depend_on_the_length_of_the_match() {
    let six = match_games("random", "random", 6, 2026);
    assert_eq!(match_games("random", "random", 4, 2026)[..], six[..4]);
}

#[test]
fn every_pairing_gets_the_same_openings() {
    let openings = |a: &str, b: &str| -> Vec<Vec<String>> {
        match_games(a, b, 6, 1)
            .iter()
            .map(|g| notations(&g.record.moves[..2]))
            .collect()
    };
    let random = openings("random", "random");
    assert_eq!(openings("tuned", "first"), random);
    assert_eq!(openings("material", "u:1/2/0.2"), random);
}

#[test]
fn alpha_beta_plays_the_games_of_minimax_at_depth_2() {
    assert_eq!(
        match_games("ab2", "tuned", 10, 1),
        match_games("mm2", "tuned", 10, 1)
    );
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn alpha_beta_plays_the_games_of_minimax_at_depth_3() {
    assert_eq!(
        match_games("ab3", "tuned", 50, 1),
        match_games("mm3", "tuned", 50, 1)
    );
    assert_eq!(
        match_games("ab3", "mm2", 50, 1),
        match_games("mm3", "mm2", 50, 1)
    );
}

#[test]
fn iterative_deepening_plays_the_games_of_alpha_beta_at_depth_3() {
    assert_eq!(
        match_games("id:d3", "tuned", 10, 1),
        match_games("ab3", "tuned", 10, 1)
    );
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn iterative_deepening_plays_the_games_of_alpha_beta_at_depths_3_and_4() {
    assert_eq!(
        match_games("id:d3", "tuned", 50, 1),
        match_games("ab3", "tuned", 50, 1)
    );
    assert_eq!(
        match_games("id:d4", "tuned", 20, 1),
        match_games("ab4", "tuned", 20, 1)
    );
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn iterative_deepening_with_quiescence_plays_the_games_of_ab3q() {
    assert_eq!(
        match_games("idq:d3", "tuned", 20, 1),
        match_games("ab3q", "tuned", 20, 1)
    );
}
