# latitude

Latitude is a library for dynamic runtime DDL based on [`sqlx`](https://github.com/launchbadge/sqlx)
and [`barrel`](https://github.com/rust-db/barrel). **_NOTE_**: This project
is in early development along with `sqlx`. Please use at your own risk and strongly
consider using a better tested and polished project such as
[Refinery](https://github.com/rust-db/refinery). There will definitely be API changes
in the near future.

Originally this was intended to to be a migration toolkit, but, it has been slimmed down. It's
unclear if we will still pursue a migration oriented API. Adding migration capability would be
a feature addition as opposed to a rewrite.
With the migratio changes this library could be considered amore portable but less
accessible alternative to `sqlx-cli`: users require less concern over understanding and
maintaining multiple SQL dialects, but, must have familiarity with Rust to get up and
running. It may be a good fit if your application is already written in Rust and either:

- you want migration compatibility across multiple databases (e.g. if you are using MySQL
  in production and SQLite for development); or

- you just like writing in the DSL over plain SQL

Please raise an issue on GitHub if you have suggestions, feedback, bug reports, or
otherwise.

### Getting Started

```toml
latitude = 0.0.1
```

### Usage

```rust
use latitude::prelude::*;

let connection = Connection::new("sqlite::memory:").await?;

table::create("users")
      .column("name", varchar(255))
      .column("age",  integer())
      .column("xyx",  boolean())
      .execute(&mut connection)
      .await?
```

License: MIT OR Apache-2.0
