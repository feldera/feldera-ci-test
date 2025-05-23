//! # Developer tutorial
//!
//! This tutorial aims to be a start at teaching Rust developers how to
//! use DBSP in their projects.
//!
//! All of the programs in this tutorial are provided as examples under
//! `examples/tutorial`.  You can run each of them with, e.g. `cargo run
//! --example tutorial1`.
//!
//! # Table of contents
//!
//! * [Introduction](#introduction)
//! * [Basic](#basics)
//!   * [Input](#input)
//!   * [Execution](#execution)
//!   * [Computation and output](#computation-and-output)
//! * [More sophisticated computation](#more-sophisticated-computation)
//!   * [Aggregation](#aggregation)
//!   * [Rolling aggregation](#rolling-aggregation)
//!   * [Joins](#joins)
//!   * [Finding months with the most
//!     vaccinations](#finding-months-with-the-most-vaccinations)
//!   * [Vaccination rates](#vaccination-rates)
//! * [Incremental computation](#incremental-computation)
//! * [Fixed-point computation](#fixed-point-computation)
//! * [Next steps](#next-steps)
//!
//! # Introduction
//!
//! Computation in DBSP is a two-stage process.  First, create a DBSP "circuit"
//! and fix its computational structure, including its inputs and outputs.
//! Second, any number of times, feed in input changes, tell DBSP to run the
//! circuits, and then read out the output changes.  A skeleton for a DBSP
//! program might look like this (the second and later steps could iterate any
//! number of times):
//!
//! ```
//! fn main() {
//!     // ...build circuit...
//!     // ...feed data into circuit...
//!     // ...execute circuit...
//!     // ...read output from circuit...
//! }
//! ```
//!
//! The following section shows the basics of how to fill in each of these
//! steps.
//!
//! # Basics
//!
//! This section shows off the basics of input, computation, and output.
//! Afterward, we'll show how to do more sophisticated computation.
//!
//! ## Input
//!
//! To process data in DBSP, we need to get data from somewhere.  The
//! `dbsp_adapters` crate in `crates/adapters` implements input and output
//! adapters for a number of formats and transports along with a server that
//! instantiates a DBSP pipeline and adapters based on a user-provided
//! declarative configuration.  In this tutorial we take a different approach,
//! instantiating the pipeline and pushing data to it directly using the Rust
//! API.  Specifically, we will parse some data from a CSV file and bring it
//! into a circuit.
//!
//! Let's work with the [Our World in Data](https://ourworldindata.org/)
//! public-domain dataset on COVID-19 vaccinations, which is available on
//! Github.  Its main data file on vaccinations is `vaccinations.csv`, which
//! contains about 168,000 rows of data.  That's a lot to stick in the DBSP
//! repo, so we've included a subset with data for just a few countries. The
//! full version of the snapshot of the data excerpted here is [freely
//! available](https://github.com/owid/covid-19-data/blob/88ab53d1081ef7651b16212658ea43bd175d572a/public/data/vaccinations/vaccinations.csv)
//! on Github.
//!
//! The vaccination data has 16 columns per row.  We will only look at three of
//! those: `location`, a country name; `date`, a date in the form `yyyy-mm-dd`;
//! and `daily_vaccinations`, the number of vaccinations given on `date` in
//! `location`.  The latter field is sometimes blank.
//!
//! Rust crates have good support for reading this data.  We can combine the
//! `csv` crate to read CSV files with `serde` for deserializing into a `struct`
//! and `time` for parsing the date field. A full program for parsing and
//! printing the data is below and in `tutorial1.rs`:

//! ```rust
//! use anyhow::Result;
//! use csv::Reader;
//! use serde::Deserialize;
//! use time::Date;
//!
//! #[allow(dead_code)]
//! #[derive(Debug, Deserialize)]
//! struct Record {
//!     location: String,
//!     date: Date,
//!     daily_vaccinations: Option<u64>,
//! }
//!
//! fn main() -> Result<()> {
//!     let path = format!(
//!         "{}/examples/tutorial/vaccinations.csv",
//!         env!("CARGO_MANIFEST_DIR")
//!     );
//!     for result in Reader::from_path(path)?.deserialize() {
//!         let record: Record = result?;
//!         println!("{:?}", record);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! If we run this, then it prints the records in `Debug` format.  Here are the
//! first few:
//!
//! ```text
//! Record { location: "England", date: 2021-01-10, daily_vaccinations: None }
//! Record { location: "England", date: 2021-01-11, daily_vaccinations: Some(140441) }
//! Record { location: "England", date: 2021-01-12, daily_vaccinations: Some(164043) }
//! Record { location: "England", date: 2021-01-13, daily_vaccinations: Some(192088) }
//! Record { location: "England", date: 2021-01-14, daily_vaccinations: Some(213978) }
//! ...
//! ```
//!
//! We want to create a DBSP circuit and bring this data into it.  We create a
//! circuit with [`RootCircuit::build`], which creates an empty circuit, calls a
//! callback that we pass it to add input and computation and output to the
//! circuit, and then fixes the form of the circuit and returns the circuit plus
//! anything we returned from our callback.  The code skeleton is like this:
//!
//! ```
//! # use anyhow::Result;
//! # use dbsp::RootCircuit;
//! fn build_circuit(circuit: &mut RootCircuit) -> Result<()> {
//!     // ...populate `circuit` with operators...
//!     Ok((/*handles*/))
//! }
//!
//! fn main() -> Result<()> {
//!     // Build circuit.
//!     let (circuit, (/*handles*/)) = RootCircuit::build(build_circuit)?;
//!
//!     // ...feed data into circuit...
//!     // ...execute circuit...
//!     // ...read output from circuit...
//! #     Ok(())
//! }
//! ```
//!
//! The natural way to bring our data into the circuit is through a "Z-set"
//! ([`ZSet`]) input stream.  A "Z-set" is a set in which each item is
//! associated with an integer weight.  In the context of changes to a data set,
//! positive weights represent insertions and negative weights represent
//! deletions.  The magnitude of the weight represents a count, so that a weight
//! of 1 represents an insertion of a single copy of a record, 2 represents two
//! copies, and so on, and similarly for negative weights and deletions.  Thus,
//! a Z-set represents changes to a multiset.
//!
//! We create the Z-set input stream inside `build_circuit` using
//! [`RootCircuit::add_input_zset`], which returns a [`Stream`] for further use
//! in `build_circuit` and a [`ZSetHandle`] for `main` to use to feed in
//! data.  Our skeleton fills in as shown below.  We're jumping the gun a bit by
//! adding a call to [`inspect`](Stream::inspect) on the `Stream`.  This method
//! calls a closure on each batch of data that passes through; we're having it
//! print the total weight in our Z-set just to demonstrate that something is
//! happening:
//!
//! ```rust
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! fn build_circuit(circuit: &mut RootCircuit) -> Result<ZSetHandle<Record>> {
//!     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//!     input_stream.inspect(|records| {
//!         println!("{}", records.weighted_count());
//!     });
//!     // ...populate `circuit` with more operators...
//!     Ok(input_handle)
//! }
//! fn main() -> Result<()> {
//!     // Build circuit.
//!     let (circuit, input_handle) = RootCircuit::build(build_circuit)?;
//!
//!     // ...feed data into circuit...
//!     // ...execute circuit...
//!     // ...read output from circuit...
//! #   Ok(())
//! }
//! ```
//!
//! The best way to feed the records into `input_handle` is to collect them into
//! a `Vec<(Record, isize)>`, where `isize` is the Z-set weight.  All the
//! weights can be 1, since we are inserting each of them.  We feed them in with
//! [`ZSetHandle::append`].  So, we can fill in `// ...feed data into
//! circuit...` with:
//!
//! ```rust
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! # Clone,
//! # Default,
//! # Debug,
//! # Eq,
//! # PartialEq,
//! # Ord,
//! # PartialOrd,
//! # Hash,
//! # SizeOf,
//! # Archive,
//! # Serialize,
//! # rkyv::Deserialize,
//! # serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! # fn build_circuit(circuit: &mut RootCircuit) -> Result<ZSetHandle<Record>> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     input_stream.inspect(|records| {
//! #         println!("{}", records.weighted_count());
//! #     });
//! #     // ...populate `circuit` with more operators...
//! #     Ok(input_handle)
//! # }
//! #
//! # fn main() -> Result<()> {
//! #     // Build circuit.
//! #     let (circuit, input_handle) = RootCircuit::build(build_circuit)?;
//! #
//!      // Feed data into circuit.
//!    let path = format!(
//!        "{}/examples/tutorial/vaccinations.csv",
//!        env!("CARGO_MANIFEST_DIR")
//!    );
//!    let mut input_records = Reader::from_path(path)?
//!        .deserialize()
//!        .map(|result| result.map(|record| Tup2(record, 1)))
//!        .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//!    input_handle.append(&mut input_records);
//!
//! #   // Execute circuit.
//! #   circuit.step()?;
//! #
//! #   // ...read output from circuit...
//! #   Ok(())
//!}
//! ```
//!
//! The compiler will point out a problem: `Record` lacks several traits
//! required for the record type of the "Z-sets".  We need `SizeOf` from the
//! `size_of` crate and `Archive`, `Serialize`, and `Deserialize` from the
//! `rkyv` crate.  We can derive all of them:
//!
//! ```
//! use rkyv::{Archive, Serialize};
//! use size_of::SizeOf;
//! use chrono::NaiveDate;
//!
//! #[derive(
//!     Clone,
//!     Default,
//!     Debug,
//!     Eq,
//!     PartialEq,
//!     Ord,
//!     PartialOrd,
//!     Hash,
//!     SizeOf,
//!     Archive,
//!     Serialize,
//!     rkyv::Deserialize,
//!     serde::Deserialize,
//! )]
//! #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! ```
//!
//! > 💡 There are two `Deserialize` traits above.  DBSP requires
//! `rkyv::Deserialize` to support distributed computations, by allowing data to
//! be moved from one host to another.  Our example uses `serde::Deserialize` to
//! parse CSV.
//!
//! ## Execution
//!
//! Our program now builds a circuit and feeds data into it.  To execute it, we
//! just replace `// ...execute circuit...` with a call to
//! [`CircuitHandle::step`]:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! # fn build_circuit(circuit: &mut RootCircuit) -> Result<ZSetHandle<Record>> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     input_stream.inspect(|records| {
//! #         println!("{}", records.weighted_count());
//! #     });
//! #     // ...populate `circuit` with more operators...
//! #     Ok(input_handle)
//! # }
//! #
//! # fn main() -> Result<()> {
//! #     // Build circuit.
//! #     let (circuit, input_handle) = RootCircuit::build(build_circuit)?;
//! #
//! #     // Feed data into circuit.
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//!      // Execute circuit.
//!      circuit.step()?;
//! #
//! #     // ...read output from circuit...
//! #     Ok(())
//! # }
//! ```
//!
//! Now, if you run our program, with `cargo run --example tutorial2`, it prints
//! `3961`, the number of records in `vaccinations.csv`.  That's because our
//! program reads an entire CSV file and feeds it as input in a single step.
//! That means that running for more steps wouldn't make a difference.  That's
//! not a normal use case for DBSP but, arguably, it's a reasonable setup for a
//! tutorial.
//!
//! ## Computation and output
//!
//! We haven't done any computation inside the circuit, nor have we brought
//! output back out of the circuit yet.  Let's add both of those to our
//! skeleton.
//!
//! Let's do just enough computation to demonstrate the concept.  Suppose we
//! want to pick out a subset of the records.  We can use [`Stream::filter`] to
//! do that.  For example, we can take just the records for locations in the
//! United Kingdom:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{OrdZSet, OutputHandle, RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(ZSetHandle<Record>, OutputHandle<OrdZSet<Record>>)> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     input_stream.inspect(|records| {
//! #         println!("{}", records.weighted_count());
//! #     });
//!     let filtered = input_stream.filter(|r| {
//!         r.location == "England"
//!             || r.location == "Northern Ireland"
//!             || r.location == "Scotland"
//!             || r.location == "Wales"
//!     });
//! #     Ok((input_handle, filtered.output()))
//! # }
//! #
//! # fn main() -> Result<()> {
//! #     // Build circuit.
//! #     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     // Feed data into circuit.
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     // Execute circuit.
//! #     circuit.step()?;
//! #
//! #     // Read output from circuit.
//! #     println!("{}", output_handle.consolidate().weighted_count());
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! We could call `inspect` again to print the results.  Instead, let's bring
//! the results out of the computation into `main` and print them there.  That's
//! just a matter of calling [`Stream::output`], which returns [`OutputHandle`]
//! to return to `main`, which can then read out the data after each step.  Our
//! `build_circuit` then looks like this:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{OrdZSet, OutputHandle, RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(ZSetHandle<Record>, OutputHandle<OrdZSet<Record>>)> {
//!     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//!     input_stream.inspect(|records| {
//!         println!("{}", records.weighted_count());
//!     });
//!     let filtered = input_stream.filter(|r| {
//!         r.location == "England"
//!             || r.location == "Northern Ireland"
//!             || r.location == "Scotland"
//!             || r.location == "Wales"
//!     });
//!     Ok((input_handle, filtered.output()))
//! }
//! #
//! # fn main() -> Result<()> {
//! #     // Build circuit.
//! #     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     // Feed data into circuit.
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     // Execute circuit.
//! #     circuit.step()?;
//! #
//! #     // Read output from circuit.
//! #     println!("{}", output_handle.consolidate().weighted_count());
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! Back in `main`, we need to update the call to [`RootCircuit::build`] so that
//! we save the new `output_handle`.  Then, after we feed in input and execute
//! the circuit, we can read the output.  For general kinds of output, it can be
//! a little tricky using `OutputHandle`, because it supports multithreaded DBSP
//! runtimes that produce one output per thread.  For Z-set output, one can just
//! call its [`consolidate`](`OutputHandle::consolidate`) method, which
//! internally merges the multiple outputs if multiple threads are in use.  To
//! print the number of records, we can just do the following:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::NaiveDate;
//! # use csv::Reader;
//! # use dbsp::utils::Tup2;
//! # use dbsp::{OrdZSet, OutputHandle, RootCircuit, ZSet, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(ZSetHandle<Record>, OutputHandle<OrdZSet<Record>>)> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     input_stream.inspect(|records| {
//! #         println!("{}", records.weighted_count());
//! #     });
//! #     let filtered = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     Ok((input_handle, filtered.output()))
//! # }
//! #
//! # fn main() -> Result<()> {
//!     // Build circuit.
//!     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     // Feed data into circuit.
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     // Execute circuit.
//! #     circuit.step()?;
//! #
//!     // ...unchanged code to feed data into circuit and execute circuit...
//!
//!     // Read output from circuit.
//!     println!("{}", output_handle.consolidate().weighted_count());
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! Now, if we run it, it prints `3961`, as before, followed by `3083`.  The
//! latter is from the `println!` in `main` and shows that we did select a
//! subset of the 3,961 total records.
//!
//! The full program is in `tutorial3.rs`.
//!
//! # More sophisticated computation
//!
//! Our program only does trivial computation, but DBSP supports much more
//! sophistication.  Let's look at some of what it can do.
//!
//! ## Aggregation
//!
//! 3,083 records is a lot.  There's so much because we've got years of daily
//! data.  Let's aggregate daily vaccinations into months, to get monthly
//! vaccinations.  DBSP has several forms of aggregation.  All of them work with
//! "indexed Z-sets" ([`IndexedZSet`]), which are Z-sets of key-value pairs,
//! that is, they associate key-value pairs with weights. Aggregation happens
//! across records with the same key.
//!
//! We will do the equivalent of the following SQL:
//!
//! ```text
//! SELECT SUM(daily_vaccinations) FROM vaccinations GROUP BY location, year, month.
//! ```
//!
//! where `year` and `month` are derived from `date`.
//!
//! To aggregate daily vaccinations over months by location, we need to
//! transform our Z-set into an indexed Z-set where the key (the index) has the
//! form `(location, year, month)` and the value is daily vaccinations (we could
//! keep the whole record but we'd just throw away most of it later).
//! To do this, we call [`Stream::index_with`], passing in a function that maps
//! a record into a key-value tuple:
//!
//! ```ignore
//!     let monthly_totals = subset
//!         .map_index(|r| {
//!             (
//!                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//!                 r.daily_vaccinations.unwrap_or(0),
//!             )
//!         })
//! ```
//!
//! We need to clone the location because it is a `String` that the records
//! incorporate by value.
//!
//! Then we can call [`Stream::aggregate_linear`], the simplest form of
//! aggregation in DBSP, to sum across months.  This function sums the output of
//! a function.  To get monthly
//! vaccinations, we just sum the values from our indexed Z-set (we have to
//! convert to `i64` because aggregation implicitly multiplies by record
//! weights):
//!
//! ```ignore
//!         .aggregate_linear(|v| *v as i64);
//! ```
//!
//! We output the indexed Z-set as before, and then in `main` print it record by
//! record:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::utils::{Tup2, Tup3};
//! # use dbsp::{OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle};
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(
//!     ZSetHandle<Record>,
//!     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, i64>>,
//! )> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     let subset = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//!     // ...
//!     Ok((input_handle, monthly_totals.output()))
//! # }
//! #
//! # fn main() -> Result<()> {
//! #     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     circuit.step()?;
//! #
//!     // ...
//!     output_handle
//!         .consolidate()
//!         .iter()
//!         .for_each(|(Tup3(l, y, m), sum, w)| println!("{l:16} {y}-{m:02} {sum:10}: {w:+}"));
//!
//!     Ok(())
//! }
//! ```
//!
//! The output looks like the following. The `+1`s are the Z-set weights.  They
//! show that each record represents an insertion of a single row:
//!
//! ```text
//! England          2021-01    5600174: +1
//! England          2021-02    9377418: +1
//! England          2021-03   11861175: +1
//! England          2021-04   11288945: +1
//! England          2021-05   13772946: +1
//! England          2021-06   10944915: +1
//! ...
//! Northern Ireland 2021-01     150315: +1
//! Northern Ireland 2021-02     317074: +1
//! ...
//! Wales            2023-01      33838: +1
//! Wales            2023-02      17098: +1
//! Wales            2023-03       8776: +1
//! ```
//!
//! The full program is in `tutorial4.rs`.
//!
//! ### Rolling aggregation
//!
//! By using a "moving average" to average recent data,
//! we can obtain a dataset with less noise due to variation from month
//! to month.   DBSP
//! provides [`Stream::partitioned_rolling_average`] for this purpose.  To
//! use it, we have to index our Z-set by time.  DBSP uses the
//! time component, which must have an unsigned integer type, to
//! define the window:
//!
//! ```ignore
//!     let moving_averages = monthly_totals
//!         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! ```
//!
//! Once we've done that, computing the moving average is easy.  Here's how we
//! get the average of the current month and the two preceding months (when
//! they're in the data set):
//!
//! ```ignore
//!         .partitioned_rolling_average(
//!             |Tup2(l, v)| (l.clone(), *v),
//!             RelRange::new(RelOffset::Before(2), RelOffset::Before(0)))
//! ```
//!
//! As the name of the function suggests, `partitioned_rolling_average`
//! computes a rolling average within a partition.  In this case, we
//! partition the data by country.  The first argument of the function
//! is a closure that splits the value component of the input indexed
//! Z-set into a partition key and a value.
//! [`partitioned_rolling_average`](`Stream::partitioned_rolling_average`)
//! returns a partitioned indexed Z-set ([`OrdPartitionedIndexedZSet`]).
//! This is just an indexed Z-set in which the key is the "partition"
//! within which averaging occurs (for us, this is the country), and the
//! value is a tuple of a "timestamp" and a value.  Note that the value
//! type has an `Option` wrapped around it.  In our case, for example, the
//! input value type was `isize`, so the output value type is `Option<isize>`.
//! The output for a given row is `None` if there are no rows in the window,
//! which can only happen if the range passed in does not include the 0 relative
//! offset (i.e. the current row).  Ours does include 0, so `None` will never
//! occur in our output.
//!
//! Let's re-map to recover year and month from the timestamp that we made and
//! to strip off the `Option`:
//!
//! ```ignore
//!         .map_index(|(l, Tup2(date, avg))| (Tup3(l.clone(), date / 12, date % 12 + 1), avg.unwrap()));
//! ```
//!
//! If we adjust the `build_circuit` return type and return value, like shown
//! below, the existing code in `main` will print it just fine.
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(
//!     ZSetHandle<Record>,
//!     OutputHandle<OrdIndexedZSet<Tup3<String, u32, u32>, i64>>,
//! )> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     let subset = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let moving_averages = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! #         .partitioned_rolling_average(
//! #               |Tup2(l, v)| (l.clone(), *v),
//! #               RelRange::new(RelOffset::Before(2), RelOffset::Before(0)))
//! #         .map_index(|(l, Tup2(date, avg))| {
//! #             (Tup3(l.clone(), date / 12, date % 12 + 1), avg.unwrap())
//! #         });
//!     // ...
//!     Ok((input_handle, moving_averages.output()))
//! }
//! #
//! # fn main() -> Result<()> {
//! #     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     circuit.step()?;
//! #
//! #     output_handle
//! #         .consolidate()
//! #         .iter()
//! #         .for_each(|(Tup3(l, y, m), sum, w)| println!("{l:16} {y}-{m:02} {sum:10}: {w:+}"));
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! The output looks like this (you can verify that the second row is the
//! average of the first two rows in the previous output, and so on):
//!
//! ```text
//! England          2021-01    5600174: +1
//! England          2021-02    7488796: +1
//! England          2021-03    8946255: +1
//! England          2021-04   10842512: +1
//! England          2021-05   12307688: +1
//! England          2021-06   12002268: +1
//! ...
//! Northern Ireland 2021-01     150315: +1
//! Northern Ireland 2021-02     233694: +1
//! ...
//! Wales            2021-01     295057: +1
//! Wales            2021-02     458273: +1
//! Wales            2021-03     584463: +1
//! ```
//!
//! The whole program is in `tutorial5.rs`.
//!
//! ### Joins
//!
//! Suppose we want both the current month's vaccination count and the moving
//! average together. With enough work, we could get them with just
//! aggregation by writing our own "aggregator" ([`Aggregator`]).  It's a
//! little easier to do a join, and it gives us a chance to show how to do that.
//! Both our monthly vaccination counts and our moving averages are indexed
//! Z-sets with the same key type.
//!
//! The first step is to take the code we've written so far and change the final
//! [`map_index`](`Stream::map_index`) on `moving_averages` to include a
//! couple of casts, so that `monthly_totals` and `moving_averages` have exactly
//! the same key type (both `(String, i32, u8)`).  The new call looks like this;
//! only the `as <type>` parts are new:
//!
//! ```ignore
//!         .map_index(|(l, Tup2(date, avg))| {
//!             (
//!                 Tup3(l.clone(), (date / 12) as i32, (date % 12 + 1) as u8),
//!                 avg.unwrap(),
//!             )
//!         });
//! ```
//!
//! Then we can use [`join_index`](`Stream::join_index`) to do the join and
//! [`inspect`](`Stream::inspect`) to print the results, as shown below.
//! Besides the streams to join, `join_index` take a closure, which it calls for
//! every pair of records with equal keys in the streams, passing in the common
//! key and each stream's value.  The closure must return an iterable collection
//! of output key-value pair tuples.  By returning a collection, the join can
//! output any number of output records per input pairing.
//!
//! We want to output a single record per input pair.  The Rust standard library
//! [`Option`] type's `Some` variant is an iterable collection that has exactly
//! one value, so it's convenient for this purpose:
//!
//! ```ignore
//!     let joined = monthly_totals.join_index(&moving_averages, |Tup3(l, y, m), cur, avg| {
//!         Some((Tup3(l.clone(), *y, *m), Tup2(*cur, *avg)))
//!     });
//!  ```
//!
//! We need to adjust the `build_circuit` return type and value and make `main`
//! print the new kind of output:
//! ```ignore
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(
//!     ZSetHandle<Record>,
//!     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, Tup2<i64, i64>>>,
//! )> {
//!     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     let subset = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let moving_averages = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! #         .partitioned_rolling_average(
//! #               |Tup2(l, v)| (l.clone(), *v),
//! #               RelRange::new(RelOffset::Before(2), RelOffset::Before(0)))
//! #         .map_index(|(l, Tup2(date, avg))| {
//! #             (
//! #                 Tup3(l.clone(), (date / 12) as i32, (date % 12 + 1) as u8),
//! #                 avg.unwrap(),
//! #             )
//! #         });
//! #     let joined = monthly_totals.join_index(&moving_averages, |Tup3(l, y, m), cur, avg| {
//! #         Some((Tup3(l.clone(), *y, *m), Tup2(*cur, *avg)))
//! #     });
//!     Ok((input_handle, joined.output()))
//! }
//!
//! fn main() -> Result<()> {
//!     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     circuit.step()?;
//! #
//!     output_handle
//!         .consolidate()
//!         .iter()
//!         .for_each(|(Tup3(l, y, m), Tup2(cur, avg), w)| {
//!             println!("{l:16} {y}-{m:02} {cur:10} {avg:10}: {w:+}")
//!         });
//!     Ok(())
//! }
//! ```
//!
//! The whole program is in `tutorial6.rs`.  If we run it, it prints both per-month
//! vaccination numbers and 3-month moving averages:
//!
//!  ```text
//! England          2021-01    5600174    5600174: +1
//! England          2021-02    9377418    7488796: +1
//! England          2021-03   11861175    8946255: +1
//! England          2021-04   11288945   10842512: +1
//! England          2021-05   13772946   12307688: +1
//! England          2021-06   10944915   12002268: +1
//! ...
//! ```
//!
//! ### Finding months with the most vaccinations
//!
//! Suppose we want to find the months when the most vaccinations occurred in
//! each country.  [`Stream`] has a ready-made method [`Stream::topk_desc`] for
//! this, which we simply pass the number of top records to keep per group.  (If
//! we only want the greatest value, rather than the top-`k` for `k > 1`, then
//! [`Stream::aggregate`] with the [`Max`] aggregator also works.)
//!
//! There is one tricky part.  To use [`topk_desc`](`Stream::topk_desc`), we
//! must re-index our data so that the country is the key (used for grouping)
//! and the number of vaccinations is the value. But, if we do that in the most
//! obvious way, we end up with just the number of vaccinations as the result,
//! whereas we probably want to know the year and month that that number of
//! occurred as well.
//!
//! One way to recover the year and month is to join against the original data.
//! We can do it without another join by defining a type that is ordered by
//! vaccination count but also contains the year and month.  Taking advantage of
//! how Rust derives [`Ord`] lexicographically, that's as simple as:
//!
//! ```rust
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//!
//! #[derive(
//!     Clone,
//!     Default,
//!     Debug,
//!     Eq,
//!     PartialEq,
//!     Ord,
//!     PartialOrd,
//!     Hash,
//!     SizeOf,
//!     Archive,
//!     Serialize,
//!     rkyv::Deserialize,
//!     serde::Deserialize,
//! )]
//! #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! struct VaxMonthly {
//!     count: u64,
//!     year: i32,
//!     month: u8,
//! }
//! ```
//!
//! We can transform our monthly totals from a `(country, year, month)` key and
//! `vaccinations` value in `country` value and `VaxMonthly` value with a call
//! to [`map_index`](Stream::map_index), then just call
//! [`topk_desc`](`Stream::topk_desc`):
//!
//! ```ignore
//!     let most_vax = monthly_totals
//!         .map_index(|(Tup3(l, y, m), sum)| {
//!             (
//!                 l.clone(),
//!                 VaxMonthly {
//!                     count: *sum as u64,
//!                     year: *y,
//!                     month: *m,
//!                 },
//!             )
//!         })
//!         .topk_desc(3);
//! ```
//!
//! Then we just adjust `build_circuit` return type and value and print the new
//! output type in `main`:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     utils::{Tup2, Tup3},
//! #     OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct VaxMonthly {
//! #     count: u64,
//! #     year: i32,
//! #     month: u8,
//! # }
//! #
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(
//!     ZSetHandle<Record>,
//!     OutputHandle<OrdIndexedZSet<String, VaxMonthly>>,
//! )> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     let subset = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as isize);
//! #     let most_vax = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), sum)| {
//! #             (
//! #                 l.clone(),
//! #                 VaxMonthly {
//! #                     count: *sum as u64,
//! #                     year: *y,
//! #                     month: *m,
//! #                 },
//! #             )
//! #         })
//! #         .topk_desc(3);
//!     // ...
//!     Ok((input_handle, most_vax.output()))
//! }
//!
//! fn main() -> Result<()> {
//! #     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut input_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     input_handle.append(&mut input_records);
//! #
//! #     circuit.step()?;
//! #
//!     // ...
//!     output_handle
//!         .consolidate()
//!         .iter()
//!         .for_each(|(l, VaxMonthly { count, year, month }, w)| {
//!             println!("{l:16} {year}-{month:02} {count:10}: {w:+}")
//!         });
//!     Ok(())
//! }
//! ```
//!
//! The complete program is in `tutorial7.rs`.  When we run it, it outputs the
//! following.  The output is sorted in increasing order, which might be a bit
//! surprising, but that's because DBSP Z-sets always iterate in that order.  If
//! it's important to produce output in another order, one could define custom
//! [`Ord`] and [`PartialOrd`] on `VaxMonthly`:
//!
//! ```text
//! England          2021-03   11861175: +1
//! England          2021-05   13772946: +1
//! England          2021-12   14801300: +1
//! Northern Ireland 2021-05     394047: +1
//! Northern Ireland 2021-04     436870: +1
//! Northern Ireland 2021-12     489059: +1
//! Scotland         2021-06    1244155: +1
//! Scotland         2021-05    1272194: +1
//! Scotland         2021-12    1388549: +1
//! Wales            2021-04     707714: +1
//! Wales            2021-12     822945: +1
//! Wales            2021-03     836844: +1
//! ```
//!
//! ## Vaccination rates
//!
//! Suppose we want to compare the population in each country with the
//! number of vaccinations given, that is, to calculate a vaccination
//! rate.  Our input data contains a `total_vaccinations_per_hundred`
//! column that reports this information, but let's calculate it ourselves
//! to demonstrate how it might be done in DBSP.
//!
//! We need to know the population in each country.  Let's add a new input
//! source for population by country.  Since that's naturally a set of key-value
//! pairs, we'll make it an indexed Z-set input source instead of a plain Z-set.
//! The first step is to make our circuit builder construct the source and
//! return its handle.  At the same time, we can prepare for our output to be an
//! indexed Z-set with `(location, year, month)` key and `(vaxxes, population)`
//! value:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     IndexedZSetHandle, OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! fn build_circuit(
//!     circuit: &mut RootCircuit,
//! ) -> Result<(
//!     ZSetHandle<Record>,
//!     IndexedZSetHandle<String, u64>,
//!     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, Tup2<i64, u64>>>,
//! )> {
//!     let (vax_stream, vax_handle) = circuit.add_input_zset::<Record>();
//!     let (pop_stream, pop_handle) = circuit.add_input_indexed_zset::<String, u64>();
//! #     let subset = vax_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let running_monthly_totals = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! #         .partitioned_rolling_aggregate_linear(
//! #             |Tup2(l, v)| (l.clone(), *v),
//! #             |vaxxed| *vaxxed,
//! #             |total| total,
//! #             RelRange::new(RelOffset::Before(u32::MAX), RelOffset::Before(0)),
//! #         );
//! #     let vax_rates = running_monthly_totals
//! #         .map_index(|(l, Tup2(date, total))| {
//! #             (
//! #                 l.clone(),
//! #                 Tup3((date / 12) as i32, (date % 12 + 1) as u8, total.unwrap()),
//! #             )
//! #         })
//! #         .join_index(&pop_stream, |l, Tup3(y, m, total), pop| {
//! #             Some((Tup3(l.clone(), *y, *m), Tup2(*total, *pop)))
//! #         });
//!     // ...
//!     Ok((vax_handle, pop_handle, vax_rates.output()))
//! }
//! #
//! # fn main() -> Result<()> {
//! #     let (circuit, (vax_handle, pop_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut vax_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     vax_handle.append(&mut vax_records);
//! #
//! #     let mut pop_records = vec![
//! #         Tup2("England".into(), Tup2(56286961u64, 1i64)),
//! #         Tup2("Northern Ireland".into(), Tup2(1893667, 1)),
//! #         Tup2("Scotland".into(), Tup2(5463300, 1)),
//! #         Tup2("Wales".into(), Tup2(3152879, 1)),
//! #     ];
//! #     pop_handle.append(&mut pop_records);
//! #
//! #     circuit.step()?;
//! #
//! #     output_handle
//! #         .consolidate()
//! #         .iter()
//! #         .for_each(|(Tup3(l, y, m), Tup2(vaxxes, pop), w)| {
//! #             let rate = vaxxes as f64 / pop as f64 * 100.0;
//! #             println!("{l:16} {y}-{m:02}: {vaxxes:9} {pop:8} {rate:6.2}% {w:+}",)
//! #         });
//! #     Ok(())
//! # }
//! ```
//!
//! The code in `main` needs to receive the additional handle and feed data into
//! it.  Let's feed in fixed data this time:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     IndexedZSetHandle, OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(
//! #     ZSetHandle<Record>,
//! #     IndexedZSetHandle<String, u64>,
//! #     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, Tup2<i64, u64>>>,
//! # )> {
//! #     let (vax_stream, vax_handle) = circuit.add_input_zset::<Record>();
//! #     let (pop_stream, pop_handle) = circuit.add_input_indexed_zset::<String, u64>();
//! #     let subset = vax_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let running_monthly_totals = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! #         .partitioned_rolling_aggregate_linear(
//! #             |Tup2(l, v)| (l.clone(), *v),
//! #             |vaxxed| *vaxxed,
//! #             |total| total,
//! #             RelRange::new(RelOffset::Before(u32::MAX), RelOffset::Before(0)),
//! #         );
//! #     let vax_rates = running_monthly_totals
//! #         .map_index(|(l, Tup2(date, total))| {
//! #             (
//! #                 l.clone(),
//! #                 Tup3((date / 12) as i32, (date % 12 + 1) as u8, total.unwrap()),
//! #             )
//! #         })
//! #         .join_index(&pop_stream, |l, Tup3(y, m, total), pop| {
//! #             Some((Tup3(l.clone(), *y, *m), Tup2(*total, *pop)))
//! #         });
//! #     Ok((vax_handle, pop_handle, vax_rates.output()))
//! # }
//! #
//! fn main() -> Result<()> {
//!     let (circuit, (vax_handle, pop_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut vax_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     vax_handle.append(&mut vax_records);
//!
//!     // ...
//!     let mut pop_records = vec![
//!         Tup2("England".into(), Tup2(56286961u64, 1i64)),
//!         Tup2("Northern Ireland".into(), Tup2(1893667, 1)),
//!         Tup2("Scotland".into(), Tup2(5463300, 1)),
//!         Tup2("Wales".into(), Tup2(3152879, 1)),
//!     ];
//!     pop_handle.append(&mut pop_records);
//!     // ...
//! #
//! #     circuit.step()?;
//! #
//! #     output_handle
//! #         .consolidate()
//! #         .iter()
//! #         .for_each(|(Tup3(l, y, m), Tup2(vaxxes, pop), w)| {
//! #             let rate = vaxxes as f64 / pop as f64 * 100.0;
//! #             println!("{l:16} {y}-{m:02}: {vaxxes:9} {pop:8} {rate:6.2}% {w:+}",)
//! #         });
//! #     Ok(())
//! # }
//! ```
//!
//! The calculation of monthly totals stays the same.  Starting from those, we
//! calculate running vaccination totals, which requires first re-indexing.
//! Then we join against the population table, which also requires a re-index
//! step:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     IndexedZSetHandle, OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(
//! #     ZSetHandle<Record>,
//! #     IndexedZSetHandle<String, u64>,
//! #     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, Tup2<i64, u64>>>,
//! # )> {
//! #     let (vax_stream, vax_handle) = circuit.add_input_zset::<Record>();
//! #     let (pop_stream, pop_handle) = circuit.add_input_indexed_zset::<String, u64>();
//! #     let subset = vax_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//!     let running_monthly_totals = monthly_totals
//!         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//!         .partitioned_rolling_aggregate_linear(
//!             |Tup2(l, v)| (l.clone(), *v),
//!             |vaxxed| *vaxxed,
//!             |total| total,
//!             RelRange::new(RelOffset::Before(u32::MAX), RelOffset::Before(0)),
//!         );
//!     let vax_rates = running_monthly_totals
//!         .map_index(|(l, Tup2(date, total))| {
//!             (
//!                 l.clone(),
//!                 Tup3((date / 12) as i32, (date % 12 + 1) as u8, total.unwrap()),
//!             )
//!         })
//!         .join_index(&pop_stream, |l, Tup3(y, m, total), pop| {
//!             Some((Tup3(l.clone(), *y, *m), Tup2(*total, *pop)))
//!         });
//! #     Ok((vax_handle, pop_handle, vax_rates.output()))
//! # }
//! ```
//!
//! And finally we adjust `main` to print the results:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     operator::time_series::{RelOffset, RelRange},
//! #     utils::{Tup2, Tup3},
//! #     IndexedZSetHandle, OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(
//! #     ZSetHandle<Record>,
//! #     IndexedZSetHandle<String, u64>,
//! #     OutputHandle<OrdIndexedZSet<Tup3<String, i32, u8>, Tup2<i64, u64>>>,
//! # )> {
//! #     let (vax_stream, vax_handle) = circuit.add_input_zset::<Record>();
//! #     let (pop_stream, pop_handle) = circuit.add_input_indexed_zset::<String, u64>();
//! #     let subset = vax_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let running_monthly_totals = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), v)| (*y as u32 * 12 + (*m as u32 - 1), Tup2(l.clone(), *v)))
//! #         .partitioned_rolling_aggregate_linear(
//! #             |Tup2(l, v)| (l.clone(), *v),
//! #             |vaxxed| *vaxxed,
//! #             |total| total,
//! #             RelRange::new(RelOffset::Before(u32::MAX), RelOffset::Before(0)),
//! #         );
//! #     let vax_rates = running_monthly_totals
//! #         .map_index(|(l, Tup2(date, total))| {
//! #             (
//! #                 l.clone(),
//! #                 Tup3((date / 12) as i32, (date % 12 + 1) as u8, total.unwrap()),
//! #             )
//! #         })
//! #         .join_index(&pop_stream, |l, Tup3(y, m, total), pop| {
//! #             Some((Tup3(l.clone(), *y, *m), Tup2(*total, *pop)))
//! #         });
//! #     Ok((vax_handle, pop_handle, vax_rates.output()))
//! # }
//! #
//! # fn main() -> Result<()> {
//! #     let (circuit, (vax_handle, pop_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//! #
//! #     let path = format!(
//! #         "{}/examples/tutorial/vaccinations.csv",
//! #         env!("CARGO_MANIFEST_DIR")
//! #     );
//! #     let mut vax_records = Reader::from_path(path)?
//! #         .deserialize()
//! #         .map(|result| result.map(|record| Tup2(record, 1)))
//! #         .collect::<Result<Vec<Tup2<Record, i64>>, _>>()?;
//! #     vax_handle.append(&mut vax_records);
//! #
//! #     let mut pop_records = vec![
//! #         Tup2("England".into(), Tup2(56286961u64, 1i64)),
//! #         Tup2("Northern Ireland".into(), Tup2(1893667, 1)),
//! #         Tup2("Scotland".into(), Tup2(5463300, 1)),
//! #         Tup2("Wales".into(), Tup2(3152879, 1)),
//! #     ];
//! #     pop_handle.append(&mut pop_records);
//! #
//! #     circuit.step()?;
//! #
//!     output_handle
//!         .consolidate()
//!         .iter()
//!         .for_each(|(Tup3(l, y, m), Tup2(vaxxes, pop), w)| {
//!             let rate = vaxxes as f64 / pop as f64 * 100.0;
//!             println!("{l:16} {y}-{m:02}: {vaxxes:9} {pop:8} {rate:6.2}% {w:+}",)
//!         });
//! #     Ok(())
//! # }
//! ```
//!
//! The complete program is in `tutorial8.rs`.  If we run it, it prints, in
//! part, the following. The percentages over 100% are correct because this data
//! counts vaccination doses rather than people:
//!
//! ```text
//! England          2021-01:   5600174 56286961   9.95% +1
//! England          2021-02:  14977592 56286961  26.61% +1
//! England          2021-03:  26838767 56286961  47.68% +1
//! England          2021-04:  38127712 56286961  67.74% +1
//! England          2021-05:  51900658 56286961  92.21% +1
//! England          2021-06:  62845573 56286961 111.65% +1
//! ...
//! ```
//!
//! # Incremental computation
//!
//! DBSP shines when data arrive item by item or in batches, because its
//! internals work "incrementally", that is, they do only as much
//! (re)computation as needed to reflect the input changes, rather than
//! recalculating everything from an empty state.
//!
//! Our examples so far have fed all of the input data into the circuit in one
//! go.  To demonstrate incremental computation, we can simulate data arriving
//! over time by dividing our CSV file into batches and separately pushing each
//! batch into an individual step of the circuit.  `tutorial9.rs` does that: it
//! is a copy of the program from [Finding months with the most
//! vaccinations](#finding-months-with-the-most-vaccinations) modified so that
//! it feeds data in at most 500 records per step.  The only changes from the
//! previous version are in `main`, which becomes:
//!
//! ```
//! # use anyhow::Result;
//! # use chrono::{Datelike, NaiveDate};
//! # use csv::Reader;
//! # use dbsp::{
//! #     utils::{Tup2, Tup3},
//! #     OrdIndexedZSet, OutputHandle, RootCircuit, ZSetHandle,
//! # };
//! # use rkyv::{Archive, Serialize};
//! # use size_of::SizeOf;
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct Record {
//! #     location: String,
//! #     date: NaiveDate,
//! #     daily_vaccinations: Option<u64>,
//! # }
//! #
//! # #[derive(
//! #     Clone,
//! #     Default,
//! #     Debug,
//! #     Eq,
//! #     PartialEq,
//! #     Ord,
//! #     PartialOrd,
//! #     Hash,
//! #     SizeOf,
//! #     Archive,
//! #     Serialize,
//! #     rkyv::Deserialize,
//! #     serde::Deserialize,
//! # )]
//! # #[archive_attr(derive(Ord, Eq, PartialEq, PartialOrd))]
//! # struct VaxMonthly {
//! #     count: u64,
//! #     year: i32,
//! #     month: u8,
//! # }
//! #
//! # fn build_circuit(
//! #     circuit: &mut RootCircuit,
//! # ) -> Result<(
//! #     ZSetHandle<Record>,
//! #     OutputHandle<OrdIndexedZSet<String, VaxMonthly>>,
//! # )> {
//! #     let (input_stream, input_handle) = circuit.add_input_zset::<Record>();
//! #     let subset = input_stream.filter(|r| {
//! #         r.location == "England"
//! #             || r.location == "Northern Ireland"
//! #             || r.location == "Scotland"
//! #             || r.location == "Wales"
//! #     });
//! #     let monthly_totals = subset
//! #         .map_index(|r| {
//! #             (
//! #                 Tup3(r.location.clone(), r.date.year(), r.date.month() as u8),
//! #                 r.daily_vaccinations.unwrap_or(0),
//! #             )
//! #         })
//! #         .aggregate_linear(|v| *v as i64);
//! #     let most_vax = monthly_totals
//! #         .map_index(|(Tup3(l, y, m), sum)| {
//! #             (
//! #                 l.clone(),
//! #                 VaxMonthly {
//! #                     count: *sum as u64,
//! #                     year: *y,
//! #                     month: *m,
//! #                 },
//! #             )
//! #         })
//! #         .topk_desc(3);
//! #     Ok((input_handle, most_vax.output()))
//! # }
//! #
//! fn main() -> Result<()> {
//!     let (circuit, (input_handle, output_handle)) = RootCircuit::build(build_circuit)?;
//!
//!     let path = format!(
//!         "{}/examples/tutorial/vaccinations.csv",
//!         env!("CARGO_MANIFEST_DIR")
//!     );
//!     let mut reader = Reader::from_path(path)?;
//!     let mut input_records = reader.deserialize();
//!     loop {
//!         let mut batch = Vec::new();
//!         while batch.len() < 500 {
//!             let Some(record) = input_records.next() else {
//!                 break;
//!             };
//!             batch.push(Tup2(record?, 1));
//!         }
//!         if batch.is_empty() {
//!             break;
//!         }
//!         println!("Input {} records:", batch.len());
//!         input_handle.append(&mut batch);
//!
//!         circuit.step()?;
//!
//!         output_handle
//!             .consolidate()
//!             .iter()
//!             .for_each(|(l, VaxMonthly { count, year, month }, w)| {
//!                 println!("   {l:16} {year}-{month:02} {count:10}: {w:+}")
//!             });
//!         println!();
//!     }
//!     Ok(())
//! }
//! ```
//!
//! The output of our new program starts out like the following.  The first
//! batch of 500 records are data for England, so the program outputs the top 3.
//! The second batch introduces some of the data for Northern Ireland, so the
//! program outputs the initial top 3.  The third batch includes a month with
//! more vaccinations than the previous record, so it "retracts" (deletes) the
//! previous 3rd-place record, indicated by the output record with `-1` weight,
//! and inserts the new record:
//!
//! ```text
//! Input 500 records:
//!    England          2021-03   11861175: +1
//!    England          2021-05   13772946: +1
//!    England          2021-12   14801300: +1
//!
//! Input 500 records:
//!    Northern Ireland 2021-03     328990: +1
//!    Northern Ireland 2021-05     394047: +1
//!    Northern Ireland 2021-04     436870: +1
//!
//! Input 500 records:
//!    Northern Ireland 2021-03     328990: -1
//!    Northern Ireland 2021-12     489059: +1
//! ```
//!
//! # Fixed-point computation
//!
//! Fixed-point computations are useful if you want to repeat a query until
//! its result does not change anymore. Then, a fixed-point is reached and
//! the query processing terminates, yielding the result. SQL provides this
//! mechanism through recursive common table expressions (CTEs).
//!
//! A classical use case for a fixed-point computation is the [transitive closure
//! of a graph](https://en.wikipedia.org/wiki/Transitive_closure#In_graph_theory).
//! We demonstrate how to compute it for a weighted, directed (acyclic) graph
//! with DBSP. We assume that the graph's edges are stored in `(from, to, weight)`
//! triples, and are interested to find out (1) which other nodes are reachable
//! from any node of the graph, (2) at what cost (by cumulating the paths'
//! weights), and (3) how many hops are required for that path (through counting
//! the paths' edges). Hence, the query's output schema is
//! `(start_node, end_node, cumulated_weight, hop_count)` quadruples.
//!
//! Next to [joins](#joins), the code also shows how to use _nested circuits_.
//! We have two execution contexts: a [root circuit](`RootCircuit`)
//! and a [child circuit](`crate::NestedCircuit`).
//! The root circuit is the one that is built by the parameter to the
//! [`RootCircuit::build`] function. The child circuit is defined by the parameter to
//! the [`ChildCircuit<()>::recursive`](`crate::ChildCircuit::recursive`) function.
//! We also make use of the [`delta0`](`crate::operator::Delta0`) operator to
//! import streams from a parent circuit into a child circuit.
//! Finally, we pick up the [incremental computation](#incremental-computation)
//! aspect from the previous section by feeding in data in two steps.
//! The first step inserts this toy graph:
//!
//! ```text
//! |0| -1-> |1| -1-> |2| -2-> |3| -2-> |4|
//! ```
//!
//! In the second step, we remove the edge from `|1| -1-> |2|` and are left
//! with this graph containing two connected components:
//!
//! ```text
//! |0| -1-> |1|      |2| -2-> |3| -2-> |4|
//! ```
//!
//! DBSP then tells us how the transitive closure computed in the first step
//! changes in response to this input change in the second step.
//!
//! ```
//! use anyhow::Result;
//! use dbsp::{
//!     operator::Generator,
//!     utils::{Tup3, Tup4},
//!     zset, zset_set, Circuit, OrdZSet, RootCircuit, Stream,
//! };
//!
//! fn main() -> Result<()> {
//!     const STEPS: usize = 2;
//!
//!     let (circuit_handle, output_handle) = RootCircuit::build(move |root_circuit| {
//!         let mut edges_data = ([
//!             zset_set! { Tup3(0_usize, 1_usize, 1_usize), Tup3(1, 2, 1), Tup3(2, 3, 2), Tup3(3, 4, 2) },
//!             zset! { Tup3(1, 2, 1) => -1 },
//!         ] as [_; STEPS])
//!         .into_iter();
//!
//!         let edges = root_circuit.add_source(Generator::new(move || edges_data.next().unwrap()));
//!
//!         // Create a base stream with all paths of length 1.
//!         let len_1 = edges.map(|Tup3(from, to, weight)| Tup4(*from, *to, *weight, 1));
//!
//!         let closure = root_circuit.recursive(
//!             |child_circuit, len_n_minus_1: Stream<_, OrdZSet<Tup4<usize, usize, usize, usize>>>| {
//!                 // Import the `edges` and `len_1` stream from the parent circuit
//!                 // through the `delta0` operator.
//!                 let edges = edges.delta0(child_circuit);
//!                 let len_1 = len_1.delta0(child_circuit);
//!
//!                 // Perform an iterative step (n-1 to n) through joining the
//!                 // paths of length n-1 with the edges.
//!                 let len_n = len_n_minus_1
//!                     .map_index(|Tup4(start, end, cum_weight, hopcnt)| {
//!                         (*end, Tup4(*start, *end, *cum_weight, *hopcnt))
//!                     })
//!                     .join(
//!                         &edges
//!                             .map_index(|Tup3(from, to, weight)| (*from, Tup3(*from, *to, *weight))),
//!                         |_end_from,
//!                          Tup4(start, _end, cum_weight, hopcnt),
//!                          Tup3(_from, to, weight)| {
//!                             Tup4(*start, *to, cum_weight + weight, hopcnt + 1)
//!                         },
//!                     )
//!                     // You can think of the `plus` operator to something
//!                     // similar to the `union` operator in SQL.
//!                     .plus(&len_1);
//!
//!                 Ok(len_n)
//!             },
//!         )?;
//!
//!         let mut expected_outputs = ([
//!             // We expect the full transitive closure in the first step.
//!             zset! {
//!                 Tup4(0, 1, 1, 1) => 1,
//!                 Tup4(0, 2, 2, 2) => 1,
//!                 Tup4(0, 3, 4, 3) => 1,
//!                 Tup4(0, 4, 6, 4) => 1,
//!                 Tup4(1, 2, 1, 1) => 1,
//!                 Tup4(1, 3, 3, 2) => 1,
//!                 Tup4(1, 4, 5, 3) => 1,
//!                 Tup4(2, 3, 2, 1) => 1,
//!                 Tup4(2, 4, 4, 2) => 1,
//!                 Tup4(3, 4, 2, 1) => 1,
//!             },
//!             // These paths are removed in the second step.
//!             zset! {
//!                 Tup4(0, 2, 2, 2) => -1,
//!                 Tup4(0, 3, 4, 3) => -1,
//!                 Tup4(0, 4, 6, 4) => -1,
//!                 Tup4(1, 2, 1, 1) => -1,
//!                 Tup4(1, 3, 3, 2) => -1,
//!                 Tup4(1, 4, 5, 3) => -1,
//!             },
//!         ] as [_; STEPS])
//!             .into_iter();
//!
//!         closure.inspect(move |output| {
//!             assert_eq!(*output, expected_outputs.next().unwrap());
//!         });
//!
//!         Ok(closure.output())
//!     })?;
//!
//!     for i in 0..STEPS {
//!         let iteration = i + 1;
//!         println!("Iteration {} starts...", iteration);
//!         circuit_handle.step()?;
//!         output_handle.consolidate().iter().for_each(
//!             |(Tup4(start, end, cum_weight, hopcnt), _, z_weight)| {
//!                 println!(
//!                     "{start} -> {end} (cum weight: {cum_weight}, hops: {hopcnt}) => {z_weight}"
//!                 );
//!             },
//!         );
//!         println!("Iteration {} finished.", iteration);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! We point out that introducing a cycle to the graph prevents this fixed-point
//! computation from terminating because then there is no fixed-point anymore.
//! To demonstrate this, we introduce a third step which feeds back in the
//! previously removed edge `|1| -1-> |2|` and, additionally, introduces
//! the edge `|4| -3-> |0|`, forming a cyclic graph. In total,
//! we obtain the following graph:
//!
//! ```text
//! |0| -1-> |1| -1-> |2| -2-> |3| -2-> |4|
//!  ^                                   |
//!  |                                   |
//!  ------------------3------------------
//! ```
//!
//! The code remains unchanged except for the changes in input data. Yet, we
//! do not find a fixed-point anymore because we can endlessly walk cycles,
//! due to ever growing cumulated weights and hop counts for already discovered
//! pairs of reachable nodes. If you want to see this in action,
//! we invite you to play around with `tutorial10.rs`.
//!
//! To fix this issue, we have to change the code to stop iterating once the
//! shortest path for each pair of nodes has been discovered. One approach to
//! achieve this is to group by each pair and use the
//! [`Min`](`crate::operator::dynamic::aggregate::Min`) aggregation operator
//! to only retain the shortest path for each pair.
//! Aggregation requires to index the stream, so there are more code changes
//! required than shown here. You can find the full code in `tutorial11.rs` but
//! the important changes take place within the child circuit:
//!
//! ```
//! # use anyhow::Result;
//! # use dbsp::{
//! #     indexed_zset,
//! #     operator::{Generator, Min},
//! #     utils::{Tup2, Tup3, Tup4},
//! #     zset_set, Circuit, NestedCircuit, OrdIndexedZSet, RootCircuit, Stream,
//! # };
//! #
//! type Accumulator =
//!     Stream<NestedCircuit, OrdIndexedZSet<Tup2<usize, usize>, Tup4<usize, usize, usize, usize>>>;
//! #
//! # fn main() -> Result<()> {
//! #     const STEPS: usize = 2;
//! #
//! #     let (circuit_handle, output_handle) = RootCircuit::build(move |root_circuit| {
//! #         let mut edges_data = ([
//! #             zset_set! { Tup3(0_usize, 1_usize, 1_usize), Tup3(1, 2, 1), Tup3(2, 3, 2), Tup3(3, 4, 2) },
//! #             zset_set! { Tup3(4, 0, 3)}
//! #         ] as [_; STEPS])
//! #         .into_iter();
//! #
//! #         let edges = root_circuit.add_source(Generator::new(move || edges_data.next().unwrap()));
//! #
//! #         let len_1 = edges
//! #             .map_index(|Tup3(from, to, weight)| (Tup2(*from, *to), Tup4(*from, *to, *weight, 1)));
//!
//! let closure = root_circuit.recursive(|child_circuit, len_n_minus_1: Accumulator| {
//!     // Import the `edges` and `len_1` stream from the parent circuit.
//!     let edges = edges.delta0(child_circuit);
//!     let len_1 = len_1.delta0(child_circuit);
//!
//!     // Perform an iterative step (n-1 to n) through joining the
//!     // paths of length n-1 with the edges.
//!     let len_n = len_n_minus_1
//!         .map_index(
//!             |(Tup2(_start, _end), Tup4(start, end, cum_weight, hopcnt))| {
//!                 (*end, Tup4(*start, *end, *cum_weight, *hopcnt))
//!             },
//!         )
//!         // We now use `join_index` instead of `join` to index the stream on node pairs.
//!         .join_index(
//!             &edges.map_index(|Tup3(from, to, weight)| (*from, Tup3(*from, *to, *weight))),
//!             |_end_from, Tup4(start, _end, cum_weight, hopcnt), Tup3(_from, to, weight)| {
//!                 Some((
//!                     Tup2(*start, *to),
//!                     Tup4(*start, *to, cum_weight + weight, hopcnt + 1),
//!                 ))
//!             },
//!         )
//!         .plus(&len_1)
//!         // Here, we use the `aggregate` operator to only keep the shortest path.
//!         .aggregate(Min);
//!
//!     Ok(len_n)
//! })?;
//!
//! #         let mut expected_outputs = ([
//! #             indexed_zset! { Tup2<usize, usize> => Tup4<usize, usize, usize, usize>:
//! #                 Tup2(0, 1) => { Tup4(0, 1, 1, 1) => 1 },
//! #                 Tup2(0, 2) => { Tup4(0, 2, 2, 2) => 1 },
//! #                 Tup2(0, 3) => { Tup4(0, 3, 4, 3) => 1 },
//! #                 Tup2(0, 4) => { Tup4(0, 4, 6, 4) => 1 },
//! #                 Tup2(1, 2) => { Tup4(1, 2, 1, 1) => 1 },
//! #                 Tup2(1, 3) => { Tup4(1, 3, 3, 2) => 1 },
//! #                 Tup2(1, 4) => { Tup4(1, 4, 5, 3) => 1 },
//! #                 Tup2(2, 3) => { Tup4(2, 3, 2, 1) => 1 },
//! #                 Tup2(2, 4) => { Tup4(2, 4, 4, 2) => 1 },
//! #                 Tup2(3, 4) => { Tup4(3, 4, 2, 1) => 1 },
//! #             },
//! #             indexed_zset! { Tup2<usize, usize> => Tup4<usize, usize, usize, usize>:
//! #                 Tup2(0, 0) => { Tup4(0, 0, 9, 5) => 1 },
//! #                 Tup2(1, 0) => { Tup4(1, 0, 8, 4) => 1 },
//! #                 Tup2(1, 1) => { Tup4(1, 1, 9, 5) => 1 },
//! #                 Tup2(2, 0) => { Tup4(2, 0, 7, 3) => 1 },
//! #                 Tup2(2, 1) => { Tup4(2, 1, 8, 4) => 1 },
//! #                 Tup2(2, 2) => { Tup4(2, 2, 9, 5) => 1 },
//! #                 Tup2(3, 0) => { Tup4(3, 0, 5, 2) => 1 },
//! #                 Tup2(3, 1) => { Tup4(3, 1, 6, 3) => 1 },
//! #                 Tup2(3, 2) => { Tup4(3, 2, 7, 4) => 1 },
//! #                 Tup2(3, 3) => { Tup4(3, 3, 9, 5) => 1 },
//! #                 Tup2(4, 0) => { Tup4(4, 0, 3, 1) => 1 },
//! #                 Tup2(4, 1) => { Tup4(4, 1, 4, 2) => 1 },
//! #                 Tup2(4, 2) => { Tup4(4, 2, 5, 3) => 1 },
//! #                 Tup2(4, 3) => { Tup4(4, 3, 7, 4) => 1 },
//! #                 Tup2(4, 4) => { Tup4(4, 4, 9, 5) => 1 },
//! #             },
//! #         ] as [_; STEPS])
//! #             .into_iter();
//! #
//! #         closure.inspect(move |output| {
//! #             assert_eq!(*output, expected_outputs.next().unwrap());
//! #         });
//! #
//! #         Ok(closure.output())
//! #     })?;
//! #
//! #     for i in 0..STEPS {
//! #         circuit_handle.step()?;
//! #     }
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! Keep in mind that this is just one way to fix the issue and that in general
//! recursive queries with aggregates are not guaranteed to converge to the
//! optimum of the aggregation function (here, the minimum function),
//! even though there exists a finite solution.
//!
//! # Next steps
//!
//! We've shown how input, computation, and output work in DBSP.  That's all
//! the basics.  A good next step could be to look through the methods available
//! on [`Stream`] for computation.
//!
//! As a final note, we used [`RootCircuit::build`] to create our circuits.
//! That method creates circuits that run in the current thread.  DBSP also
//! provides a multithreaded runtime.  To run our circuit in 4 worker threads
//! instead of in the current thread is as simple as importing
//! [`dbsp::Runtime`](`Runtime`) and then changing
//!
//! ```ignore
//! let (circuit, (/*handles*/)) = RootCircuit::build(build_circuit)?;
//! ```
//!
//! to:
//!
//! ```ignore
//! let (mut circuit, (/*handles*/)) = Runtime::init_circuit(4, build_circuit)?;
//! ```
use crate::{
    operator::{Aggregator, Max},
    CircuitHandle, IndexedZSet, OrdPartitionedIndexedZSet, OutputHandle, RootCircuit, Runtime,
    Stream, ZSet, ZSetHandle,
};
