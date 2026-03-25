# eBPF Blockchain Project

## Redefining Node Architecture through Kernel-Level Observability

### Executive Summary
The eBPF Blockchain project is an R&D initiative focused on overcoming the performance and transparency limitations inherent in current decentralized network architectures. By integrating **libp2p** for P2P communication and **eBPF (Extended Berkeley Packet Filter)** for kernel-level management, we are transforming blockchain nodes from passive applications into active, self-monitoring, and self-healing systems.

This architecture redefines the node as a high-performance entity that leverages OS-level primitives to ensure security, efficiency, and visibility at scale.

---

### Core Architectural Pillars
*   **libp2p (Networking):** Abstraction of the network layer using **QUIC** for low-latency transport and **Gossipsub v1.1** for efficient, secure block propagation.
*   **eBPF (Kernel Integration):** Shared memory (eBPF Maps) between kernel and user space for real-time traffic control, latency metrics, and instant security enforcement (blacklist management) with near-zero overhead.
*   **Security:** Native integration of fuzzing (AFL++) and dynamic security enforcement at the kernel level via XDP/TC programs.

---

### Project Documentation
This repository contains extensive documentation for both researchers and engineers. Use the following guide to navigate the knowledge base:

| Documentation File | Purpose | Target Audience |
| :--- | :--- | :--- |
| `docs/README.md` | Central index and entry point for all documentation. | Everyone |
| `docs/QUICKSTART.md` | Step-by-step guide to launch a functional node in 5 minutes. | Engineers |
| `docs/LAB-GUIDE.md` | Deep dive into architecture, P2P logic, and eBPF integration. | CTO / Technical Lead |
| `docs/TUTORIAL-AYA.md` | Specific guide on using Aya-rs for eBPF development. | Rust Developers |
| `lxc-install.md` | Infrastructure setup guide using LXC containers. | DevOps / SysAdmin |
| `planing.md` | Current roadmap, task tracking, and future milestones. | Project Managers |

---

### Key Technical References
For a complete understanding of the design philosophy, goals, and technical specifications, refer to the **RFC 001: Arquitectura de Blockchain con Observabilidad Nativa (eBPF) y Networking P2P (libp2p)** document within the internal project wiki.
