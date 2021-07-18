# Landscape Water Levels

## Introduction

This is a web application that simulates the flow of water across different segments of a landscape. The idea is that a user can introduce a comma separated list of landscape segment levels and a number of hours to simulate, and start a simulation. The user will see the progress of the simulation and interact with it to pause, continue, or fast forward.

![](images/screenshot.png)

The main goal for me was to put in practice my skills with Rust, and build a WebSocket based service that required interactivity from the users. To that aim I decided to use the great [tokio-tungstenite] crate and leverage concurrency with [tokio], [futures] and `async/await`.

To make the exercise more visual and interactive, I also took the chance to learn a new technology and build a web fronted using [ELM]. Aside from this being the first time in my life working with ELM or writing Haskell code, I am not a professional frontend developer, so please take it with a grain of salt.

[tokio-tungstenite]: https://github.com/snapview/tokio-tungstenite/tree/master/examples
[tokio]: https://tokio.rs/
[futures]: https://docs.rs/futures/0.3.15/futures/
[ELM]: https://elm-lang.org/

## Software design

The application is divided in different parts:

- The **user interface** is a single page application that connects with the server through a WebSocket (see [frontend](frontend)).
- The **networking** part deals with WebSocket connections (see [main.rs](src/main.rs))
- The **protocol** part deals with the user interactions following a simple protocol that allows to start a simulation, pause and resume it, and fast forward it (see [protocol.rs](src/protocol.rs)).
- The **simulation** part encapsulates the simulation logic (see [simulation.rs](src/simulation.rs)).
- The **water_flow** part deals with the flow of water through a landscape (see [water_flow.rs](src/water_flow.rs)).

![](images/design.png)

The protocol is abstracted out from the networking by using plain `futures` `Sink`s and `Stream`s that transport messages. To allow easy unit testing of the protocol, the internal events sent to itself (for example to signal that a new step needs to be simulated after a certain period of time) are delegated to a `Sink` and a `Stream`. They are created from a single `mpsc::channel` in runtime, but a couple of different channels for unit testing.

## The algorithm and its complexity

### Internal representation of a landscape

One of the key points of the algorithm is the representation of the `sinks` in the landscape as a hierarchical tree of nodes, each one containing information about:

- what region and levels of the landscape it covers.
- how much water it can contain.
- how much water it contains.
- its hierarchical relationship with other sinks, which determines how the water will flow and with what proportions.

For example, for the landscape `[6, 4, 5, 9, 9, 2, 6, 5, 9, 7]`, these are the nodes representing the sinks and the flow of water:

![](images/hierarchy1.png)

Which results in the following tree:

![](images/hierarchy2.png)

If we had some water accumulated in the sinks like in the following example:

![](images/hierarchy3.png)

This would be the internal state of the tree:

![](images/hierarchy4.png)


### Overall idea of the algorithm

The main parts of the algorithm are:

1. Creating the hierarchy of sinks
2. Filling the sinks bottom up
3. Spilling water into the siblings
4. Flooding the water contained in the sinks into the segments

Once a hierarchy of sinks has been created (1), it can be reused for simulating the rain multiple times (2 - 4).

The water will always flow from the root sink to the leafs, but it will fill the sinks from the bottom up.

![](images/algorithm1.png)

When one of the children sinks fills up, the excess of water will spill into the sibling sinks.

![](images/algorithm2.png)

The spilling of water into the siblings will happen in both directions and following the right proportions and the available capacity.

![](images/algorithm3.png)

### Complexity

**TODO**

**Inputs:**

- **L** = The landscape segment levels for all the segments
- **h** = The number of hours to simulate

**Output:**

- The level of the water in the different segments

**Given:**


### Test Cases

Here you will find some interesting test sets:

**This will spill the water from the center sinks towards the sides.**

```
0,9,1,9,2,9,3,9,4,9,5,9,6,9,7,9,8,9,8,9,7,9,6,9,5,9,4,9,3,9,2,9,1,9,0
```

![](images/testcase1.png)

**This requires the water to spill and fill recursively down the hierarchy**

You can observe how the segments 7 and 13 spill into the center without filling 9 and 11 until 10 has not been filled to their level.

```
4,9,5,9,6,9,7,9,8,2,8,9,7,9,6,9,5,9,4
```

![](images/testcase2.png)

## Development instructions

The source code is hosted in *Github*, where on every push to master, [some Github Actions](.github/workflows/main.yaml) are run, to analyse the code (format and linting), run the tests (mostly unit tests, but also an integration test), build the artifacts (frontend assets, backend binary, and docker image), and deploy to [fly.io]

All the automated processes can be run using the tool [make] and it is configured in a couple of Makefiles ([one global](Makefile), and [the other for the frontend](frontend/Makefile)).

Some of the most used commands are:

- `make local-all` to format, lint and test the code.
- `make run` to run the server locally.
- `cargo watch -x 'test -- --nocapture'` to run the test automatically while you alternate between writing tests and code.
- `make frontend-build` to build the assets for the frontend.
- `fly-deploy` to deploy to fly.io (in real life there should be different environments configured with different API tokens to avoid mistakes)

[fly.io]: https://fly.io/
[make]: https://www.gnu.org/software/make/manual/make.html
