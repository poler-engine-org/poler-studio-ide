# Optimizing a Rust GPU matmul kernel

Optimizing a Rust GPU matmul kernel | Rust GPU

Skip to main content

Rust GPU

Docs

Rust GPU

Docs

Blog

Ecosystem

Changelog

GitHub

← Back to main menu

2025

Rust CUDA August 2025 project update

Rust running on every GPU

Porting GPU shaders to Rust 30x faster with AI

Rust CUDA May 2025 project update

Shadertoys ported to Rust GPU

Optimizing a Rust GPU matmul kernel

November 25, 2024

· 19 min read

Christian Legnitto

Rust GPU and Rust CUDA maintainer 

I read the excellent post 

We'll follow Zach's original post closely, comparing and contrasting using Rust vs the WGSL and Typescript from his post.

At the end, I'll show some unique benefits of using Rust on the GPU.

A big thank you to 

tip

The complete runnable code can be 

What is Rust GPU? 

Rust GPU

These Rust GPU programs are then compiled into 

For more details, check out the 

How does Rust GPU work? 

Rust GPU focuses purely on compiling your Rust code into SPIR-V. This compiled code is what the GPU executes. However, Rust GPU doesn't dictate how you handle CPU-to-GPU communication or data transfer. You're free to choose a host CPU library written in whatever language that fits your project. Some popular options in Rust include:

ash

vulkano

wgpu

But again, you don't 

What will we use? 

In Zach's post, he writes his GPU programs in 

We'll take a different approach: writing GPU programs in Rust via Rust GPU and managing everything—including the CPU-side code—in Rust. This means both the GPU programs and the code controlling them will be written in the same language. If you are familiar with web programming, what we are doing is conceptually similar to Javascript running on both the server and the client.

Using Rust for both CPU and GPU has advantages, like consistent tooling and shared code. But it also means we need to be clear about which code runs where. I've tried to make sure this distinction is easy to follow.

To handle communication between our code on the CPU and GPU, we'll use 

By using Rust GPU and 

GPU program basics 

The smallest unit of execution is a thread, which executes the GPU program.

Workgroups are groups of threads: they are grouped together and run in parallel (they're called 

We can dispatch many of these workgroups at once. CUDA calls this a grid (which is made of thread blocks).

Workgroups and dispatching workgroups are defined in 3D. The size of a workgroup is defined by 

Writing the kernel 

Kernel 1: Naive kernel 

The simplest way to compute a dot product between matrix A and B and write to matrix C is for each row in A (of shape M), iterate over the columns of A (of shape K) and multiply by the corresponding value of B.

Here, we have our first difference from Zach's post. In WGSL, you must define inputs at the top-level scope:

WGSL

And then write your kernel:

WGSL

With Rust GPU, we specify the inputs as arguments to the kernel and configure them with 

Naive kernel with Rust GPU

This code looks like normal Rust code but 

There are a couple of things to note about the Rust implementation:

The kernel uses the regular Rust 

Libraries are imported via 

We're importing a vendored copy of 

The inner loop ( 

Read-only inputs are immutable references ( 

What's with all the 

Rust defines 

On most GPU hardware, 

This explicitness might seem tedious but it is one of the ways Rust prevents subtle bugs. It forces you to think about whether your assumptions about hardware alignment and pointer sizes are correct, making your code more portable and reliable.

info

Matrix multiplication is a pathological case with lots of indexing and row and column calculations. Most Rust GPU code does not have nearly as many 

Dispatching workgroups 

Each workgroup, since it's only one thread ( 

To calculate the full matrix, we need to launch as many entries as there are in the 

Calculating on the CPU how many workgroup dispatches are needed

The 

Using wgpu on the CPU to dispatch workgroups to the GPU

warning

This code appears more complicated than it needs to be. I abstracted the CPU-side code that talks to the GPU using generics and traits so I could easily slot in different kernels and their settings while writing the blog post.

You could just hardcode the value for simplicity.

Kernel 2: Moarrr threads! 

With the first kernel, we're only able to compute small square matrices due to limits on the number of workgroups you can dispatch at once.

Since we're launching one workgroup per entry, a 256x256 matrix is larger than our limit!

Remember this line?

We can reduce the number of dispatched workgroups by increasing the number of threads per workgroup!

If we update our GPU code

we can reduce the number of total dispatched workgroups per dimension:

Calculating how many workgroup dispatches are needed on the CPU

With these two small changes we can handle larger matrices without hitting hardware workgroup limits.

Kernel 3: Calculating with 2D workgroups 

However, doing all the computation in "1 dimension" still limits the matrix size we can calculate.

Although we don't change much about our code, if we distribute our work in 2 dimensions we're able to bypass these limits and launch more workgroups that are larger. This allows us to calculate a 4096x4096 matmul.

We update our 

2D workgroup kernel with Rust GPU

And we need to tweak the workgroup dispatch count calculation on the CPU as we are in 2D now and using the 

Calculating how many workgroup dispatches are needed on the CPU

Kernel 4: Kernel tiling 

Another thing to consider is how much work each thread does.

Up to now, each thread only computes one entry. But there is some overhead to launching each workgroup versus computing more than 1 element per thread!

If calculating more elements per thread is faster than the overhead to launch each workgroup, we should see a big speedup.

To do so, we calculate 4 results per thread (e.g. a 1x4 Tile).

Tiling kernel with Rust GPU

The kernel looks roughly the same as before except we've unrolled the computation and are calculating 

But this code is kinda gross...it looks like the opaque GPU code we are used to. Let's make it nice!

Tiling kernel using loops with Rust GPU

Much better.

We can take this a step further and calculate 2D results per thread! Instead of calculating 4 elements per single row, we can calculate 4 elements for 4 rows (e.g. a 2D tile).

2D tiling kernel with Rust GPU

Each thread now calculates a 4x4 grid of the output matrix and we see a slight improvement over the last kernel.

To stay true to the spirit of Zach's original blog post, we'll wrap things up here and leave the "fancier" experiments for another time.

A note on performance 

I didn't include performance numbers as I have a different machine than Zach. The complete runnable code can be 

tip

You can also check out real-world projects using Rust GPU such as 

Reflections on porting to Rust GPU 

Porting to Rust GPU went quickly, as the kernels Zach used were fairly simple. Most of my time was spent with concerns that were not specifically about writing GPU code. For example, deciding how much to abstract vs how much to make the code easy to follow, if everything should be available at runtime or if each kernel should be a compilation target, etc. 

My background is not in GPU programming, but I do have Rust experience. I joined the Rust GPU project because I tried to use standard GPU languages and knew there must be a better way.

Writing these GPU kernels felt like writing any other Rust code (other than debugging, more on that later) which is a huge win to me. Not just the language itself, but the entire development experience.

Rust-specific party tricks 

Rust lets us write code for both the CPU and GPU in ways that are often impossible—or at least less elegant—with other languages. I'm going to highlight some benefits I experienced while working on this blog post.

Shared code across GPU and CPU 

In GPU programming, we often need to pass data between the CPU and GPU. For example, our GPU kernel expects a 

We create an instance of 

Creating the Dimensions struct on the CPU and writing it to the GPU

This means the code on the CPU and GPU need to agree on the definition of 

In many GPU programming ecosystems, this would involve manually keeping the definitions in sync across different languages—one for the CPU, one for the GPU. This is tedious and error-prone.

With Rust, it's straightforward: we move the 

This approach eliminates duplication and guarantees consistency. If we need to make changes, those changes propagate to both the CPU and GPU automatically, reducing the risk of mismatches and making refactoring far safer.

This kind of consistency across CPU and GPU is something you don't often see in other GPU programming ecosystems. Bespoke codegen solutions are often created to accomplish the same thing Rust has built in.

Running and debugging shaders on the CPU 

GPU code can be notoriously hard to debug. While developing this kernel, I ran into a bug I couldn't figure out. GPU debugging tools are limited and 

With Rust GPU, this was straightforward. By using standard Rust 

Here's what it looks like in practice using the 2D tiling kernel from before:

The logic in the kernel hasn't changed, it is exactly the same as the GPU-only code from before.

You'll also notice that on the GPU it uses 

This is enabled by the standard Rust ecosystem tooling around dependencies:

Cargo.toml

Testing the kernel in isolation is useful, but it does not reflect how the GPU executes it with multiple invocations across workgroups and dispatches. To test the kernel end-to-end, I needed a test harness that simulated this behavior on the CPU.

Building the harness was straightforward due to Rust. By enforcing the same invariants as the GPU I could validate the kernel under the same conditions the GPU would run it:

warning

Again, this code appears more complicated than it needs to be. I abstracted the CPU testing harness code using generics and traits so I could easily slot in different kernels and their settings while writing the blog post.

You could just call the kernel function directly in nested loops.

Tests 

By moving the kernel code to the CPU, I could write tests that ran quickly and entirely on the CPU. This eliminated the need to serialize tests and offload them to the GPU (which is a shared and limited resource).

This approach has several benefits. First, it significantly reduced the feedback loop during development, allowing me to catch issues faster. Second, it ensured the tests could be run in any environment where the Rust toolchain is available—no GPU required. This is especiallly relevant in CI environments such as Github Actions that do not have a GPU by default.

For example, my test for a small matrix multiplication kernel running in the harness on the CPU looked like this:

Benchmarks 

I wanted to run benchmarks similar to those in the original blog post. Because I was using Rust, this was simple. I used 

This required no new tools or workflows. The tools I already knew worked seamlessly. More importantly, this approach benefits anyone working on the project. Any Rust engineer can run these benchmarks with no additional setup— 

Formatting 

Rust GPU code is formatted with 

Lint 

Linting GPU code in Rust works the same way as for CPU code. Running 

Documentation 

Writing doc comments and running 

But wait, there's more! 

The kernel in Zach's blog post is intentionally simple. That makes it easy to follow, but it also means the Rust code looks very similar to WGSL. While this is fine for an introductory example, it doesn't demonstrate Rust's real strengths for GPU programming. These strengths—reusing existing libraries, traits, enums, generics, and more—become much more important as projects grow in complexity.

Leverage the existing Rust ecosystem 

Rust's 

Traits 

Traits are one of Rust's most powerful tools and they work with Rust GPU. Traits let you define zero-cost reusable type-safe behavior. For example, if you have multiple kernels for different matrix multiplication strategies, you can define a 

Enums and zero-sized types 

GPU code is notoriously hard to read, but Rust's enums and zero-sized types (ZSTs) can make it much more understandable. Enums let you explicitly encode states or modes. For example, you can define tiling strategies or precision levels using enums instead of relying on constants or magic numbers.

ZSTs take this further by encoding configurations directly into the type system. For example, you could represent different kernel configurations as ZSTs. This approach ensures invalid configurations are impossible, improving both readability and safety.

Generics 

Generics are another feature missing from this kernel but are a powerful tool in Rust GPU. They allow you to write flexible kernels that work across different data types or memory layouts. For instance, you can write a single function that supports both 

Error handling with 

Rust GPU also supports error handling using 

Iterators 

Rust's iterators don't appear in this kernel, but they're another way Rust GPU simplifies complex logic. Instead of manual loops with indices, you can use iterators to express your logic more clearly.

Iterators reduce the chance of off-by-one errors and make the intent of the code much clearer.

Rust GPU's support for iterators is not complete but we are looking to improve it in the future.

Conditional compilation 

While I briefly touched on it a couple of times, this kernel doesn't really show the full power of conditional compilation. With 

Come join us! 

Rust GPU only recently became a 

Footnotes 

Why not CUDA? That is covered by 

Technically 

Technically 

Technically 

Tags:

demo

code

performance

Edit this page

Newer post Rebooting the Rust CUDA project

Older post Welcoming two new Rust GPU maintainers

What is Rust GPU?

How does Rust GPU work?

What will we use?

GPU program basics

Writing the kernel

Kernel 1: Naive kernel

Kernel 2: Moarrr threads!

Kernel 3: Calculating with 2D workgroups

Kernel 4: Kernel tiling

A note on performance

Reflections on porting to Rust GPU

Rust-specific party tricks

Shared code across GPU and CPU

Running and debugging shaders on the CPU

Tests

Benchmarks

Formatting

Lint

Documentation

But wait, there's more!

Leverage the existing Rust ecosystem

Traits

Enums and zero-sized types

Generics

Error handling with Result

Iterators

Conditional compilation

Come join us!
