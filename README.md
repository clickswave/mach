<p align="center">
  <img src="https://github.com/clickswave/mach/blob/main/static/cover.png?raw=true" alt="Mach Cover" width="100%">
</p>

> **mach is now developed in the [Crossfyre toolchain](https://github.com/clickswave/crossfyre-toolchain), under the name ambulo.** This repository keeps the history of the standalone tool. Install the current version with `cargo install --git https://github.com/clickswave/crossfyre-toolchain.git ambulo`, or the whole toolchain with the installer in the [toolchain README](https://github.com/clickswave/crossfyre-toolchain#install). New versions, fixes and issues belong in the toolchain repository.

**MACH** is a fast web fuzzing and asset discovery tool that helps security testers find hidden endpoints, services and vulnerabilities on the systems they are authorized to test.  
It’s designed with **speed**, **precision**, and **stateful tracking** in mind.

---

## ✨ Features

- 🚀 **Fast Fuzzing**: High-performance scanning engine optimized for speed.
- 🔍 **Smart Discovery**: Finds hidden endpoints, subdomains, and assets.
- 🛠️ **Stateful Engine**: Keeps track of discovered assets efficiently.
- 📊 **Visual Output**: Clear results and structured reports.
- ⚡ **Diverse & Powerful**: Built for security testing workflows.

---

## 📸 Screenshots

<p align="center">
  <img src="https://github.com/clickswave/mach/blob/main/static/ss_1.png?raw=true" alt="Mach Screenshot 1" width="80%">
</p>
<p align="center">
  <img src="https://github.com/clickswave/mach/blob/main/static/ss_2.png?raw=true" alt="Mach Screenshot 2" width="80%">
</p>
<p align="center">
  <img src="https://github.com/clickswave/mach/blob/main/static/ss_3.png?raw=true" alt="Mach Screenshot 3" width="80%">
</p>

---

## 🚀 Getting Started

```bash
# Clone the repository
git clone https://github.com/clickswave/mach.git
cd mach

# Build
cargo build --release

# Run Mach
./target/release/mach --help
