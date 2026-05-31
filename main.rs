use rand::Rng;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

const NUM_BUCKETS: usize = 100;
const PLAYERS_PER_MATCH: usize = 10;
const TEAM_SIZE: usize = 5;

#[derive(Clone, Debug)]
pub struct Player {
    pub id: usize,
    pub skill: usize, 
    pub queued_at: Instant,
}

pub struct Metrics {
    pub total_queued: AtomicUsize,
    pub total_matches: AtomicUsize,
    pub players_in_pool: AtomicUsize,
}

pub struct Matchmaker {
    buckets: Vec<Mutex<VecDeque<Player>>>,
    pub metrics: Arc<Metrics>,
}

impl Matchmaker {
    pub fn new() -> Arc<Self> {
        let mut buckets = Vec::with_capacity(NUM_BUCKETS);
        for _ in 0..NUM_BUCKETS {
            buckets.push(Mutex::new(VecDeque::new()));
        }
        Arc::new(Self {
            buckets,
            metrics: Arc::new(Metrics {
                total_queued: AtomicUsize::new(0),
                total_matches: AtomicUsize::new(0),
                players_in_pool: AtomicUsize::new(0),
            }),
        })
    }

    pub fn queue_player(&self, player: Player) {
        let bucket_idx = player.skill.min(NUM_BUCKETS - 1);
        self.buckets[bucket_idx].lock().unwrap().push_back(player);
        
        self.metrics.total_queued.fetch_add(1, Ordering::Relaxed);
        self.metrics.players_in_pool.fetch_add(1, Ordering::Relaxed);
    }

    pub fn process(&self) {
        for i in 0..NUM_BUCKETS {
            self.try_match(i);
        }
    }

    fn try_match(&self, base_bucket: usize) {
        let mut search_radius = 0;
        
        {
            let bucket = self.buckets[base_bucket].lock().unwrap();
            if let Some(oldest) = bucket.front() {
                let wait_time = oldest.queued_at.elapsed().as_secs();
                search_radius = wait_time as usize; 
            } else {
                return;
            }
        }

        let min_bucket = base_bucket.saturating_sub(search_radius);
        let max_bucket = (base_bucket + search_radius).min(NUM_BUCKETS - 1);

        let mut locked_buckets: Vec<_> = (min_bucket..=max_bucket)
            .map(|idx| self.buckets[idx].lock().unwrap())
            .collect();

        let total_available: usize = locked_buckets.iter().map(|b| b.len()).sum();
        if total_available < PLAYERS_PER_MATCH {
            return;
        }

        let mut matched_players = Vec::with_capacity(PLAYERS_PER_MATCH);
        for bucket in locked_buckets.iter_mut() {
            while matched_players.len() < PLAYERS_PER_MATCH && !bucket.is_empty() {
                matched_players.push(bucket.pop_front().unwrap());
            }
        }

        self.metrics.players_in_pool.fetch_sub(PLAYERS_PER_MATCH, Ordering::Relaxed);
        self.metrics.total_matches.fetch_add(1, Ordering::Relaxed);

        Self::balance_teams(matched_players);
    }

    fn balance_teams(mut players: Vec<Player>) {
        players.sort_by_key(|p| p.skill);
        
        let mut team_a = Vec::with_capacity(TEAM_SIZE);
        let mut team_b = Vec::with_capacity(TEAM_SIZE);

        for (i, p) in players.into_iter().enumerate() {
            if i % 4 == 0 || i % 4 == 3 {
                team_a.push(p);
            } else {
                team_b.push(p);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Starting 5v5 Matchmaking Engine Simulation...");
    let matchmaker = Matchmaker::new();
    
    for _ in 0..4 {
        let mm = Arc::clone(&matchmaker);
        tokio::spawn(async move {
            loop {
                mm.process();
                sleep(Duration::from_millis(5)).await;
            }
        });
    }

    let mm_gen = Arc::clone(&matchmaker);
    tokio::spawn(async move {
        // We generate the random number inside the loop directly on the fly
        // so it doesn't cross the .await boundary!
        for i in 1..=10_000 {
            mm_gen.queue_player(Player {
                id: i,
                skill: rand::thread_rng().gen_range(0..NUM_BUCKETS), 
                queued_at: Instant::now(),
            });
            
            if i % 500 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }
        println!("All 10,000 players have entered the queue.");
    });

    let mm_metrics = Arc::clone(&matchmaker);
    for tick in 1..=10 {
        sleep(Duration::from_secs(1)).await;
        let queued = mm_metrics.metrics.total_queued.load(Ordering::Relaxed);
        let pool = mm_metrics.metrics.players_in_pool.load(Ordering::Relaxed);
        let matches = mm_metrics.metrics.total_matches.load(Ordering::Relaxed);
        println!("[Tick {}s] Players Queued: {} | Waiting in Pool: {} | Matches Formed: {}", 
                 tick, queued, pool, matches);
        
        if pool == 0 && queued == 10_000 {
            println!("Simulation complete: Pool is empty, all matches formed successfully.");
            break;
        }
    }
}
