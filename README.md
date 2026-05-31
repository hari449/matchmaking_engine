# 5v5 Matchmaking Engine Architecture

This repository contains a high-performance, thread-safe 5v5 game matchmaking engine written in Rust. The system is designed to hold waiting players in memory and group them into balanced teams while optimizing for fast processing times and high match quality.

## Engineering Challenges Addressed

**1. The Core Algorithm (Latency vs. Match Quality)**
* **Approach:** Players are partitioned upon entry into one of 100 "Skill Buckets" (represented as a `VecDeque`). This effectively functions as a hash map with O(1) insertion time. When searching for a match, the engine localizes its search to adjacent buckets, prioritizing exact skill matches first before looking outward.

**2. Thread-Safe State & Atomic Eviction**
* **Approach:** A single, giant Mutex across the whole player pool would cripple throughput. Instead, the engine uses **fine-grained locking** by applying a `Mutex` to *each individual skill bucket*. Multiple worker threads can scan completely different skill ranges simultaneously without blocking one another.
* **Deadlock Prevention:** When a match requires gathering players across multiple buckets (due to wait-time relaxation), the worker locks the necessary buckets in **strict ascending index order**. This completely prevents circular wait conditions (deadlocks) when concurrent threads scan overlapping ranges.

**3. Time-Based Constraint Relaxation**
* **Approach:** Before locking multiple buckets, a worker peeks at the front of the target bucket. Because queues are FIFO, the front player is always the oldest. The worker calculates `wait_time = queued_at.elapsed()` and dynamically expands the `search_radius`. A player waiting 5 seconds expands the search 5 buckets wide, ensuring extreme outliers are eventually matched and preventing starvation.

**4. Team Balance Optimization**
* **Approach:** Once 10 players are atomically pulled from the pool, they are sorted by skill. To distribute the skill sum evenly, the engine uses a **Snake Draft (Zig-Zag) distribution** (ABBAABBAAB). This ensures that if the sorted players are ranked 1 to 10, Team A gets {1, 4, 5, 8, 9} and Team B gets {2, 3, 6, 7, 10}, resulting in near-identical total team skill values.

**5. Low-Latency Health Metrics**
* **Approach:** Wrapping an analytics object in a Mutex would slow down the matching loop. The engine utilizes `std::sync::atomic::AtomicUsize` for metric tracking. Workers update match counts and pool sizes using `Ordering::Relaxed`, providing hardware-level atomic operations with near-zero latency overhead.

## Complexity Analysis
* **Time Complexity:** * **Insertion:** O(1). Hashing to an array index and pushing to a `VecDeque`.
  * **Matching:** O(K) where K is the `search_radius`. Because players are pre-sorted into buckets, workers do not have to iterate through the entire pool (O(N)). Sorting the matched team of 10 is technically O(M log M), but since M is statically 10, it effectively reduces to O(1).
* **Space Complexity:** O(N) where N is the number of players currently waiting in the pool.

## Scaling Strategies
To scale this further for millions of concurrent users:
1. **Distributed State:** Move the in-memory buckets to a distributed cache like Redis, utilizing Redis Lua scripts to execute the atomic multi-bucket extraction.
2. **Horizontal Worker Scaling:** Decouple the matching logic from the ingestion logic using a message broker (like Kafka). Ingestion services push tickets to Redis, while stateless matching microservices continuously run the radial search algorithm.

## Note on Simulation Script
To ensure zero network latency and demonstrate maximum performance under load for this assignment, the Simulation Script is integrated directly into the Rust `main()` function. Running `cargo run` automatically spins up the engine and injects 10,000 concurrent players to form 1,000 matches.
