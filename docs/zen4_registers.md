# Zen4 Register Reference (57647_zen4_sog.txt)

## 1. Architectural and scalar registers

### 1.1 GPR set and addressing rules
- Zen4 executes AMD64 instructions through macro/micro-ops with fixed fields and a deliberately large register set to raise performance and keep the core extensible (`57647_zen4_sog.txt:514` `57647_zen4_sog.txt:518`).
- Any fastpath-single instruction with a memory operand becomes a fastpath double when its addressing form uses two register sources (base+index or base+index+displacement), so compilers should avoid those forms whenever possible (`57647_zen4_sog.txt:525` `57647_zen4_sog.txt:526`).
- Each SMT thread exposes six fully symmetric core performance counters, providing an independent bank of counter registers per thread (`57647_zen4_sog.txt:431`).

### 1.2 Zeroing idioms for GPRs
- The core recognizes "zeroing idioms" that clear a register without loading an immediate and therefore break dependency chains (`57647_zen4_sog.txt:1166`).
- `XOR reg, reg` clears the destination register and all flags in zero cycles (`57647_zen4_sog.txt:1178`).
- `SUB reg, reg` does the same, also with zero latency (`57647_zen4_sog.txt:1180`).
- `CMP reg, reg` leaves only ZF set and clears the other flags without delay (`57647_zen4_sog.txt:1182`).
- `SBB reg, reg` copies the zero-extended Carry flag into the register without depending on the old value, taking one cycle (`57647_zen4_sog.txt:1184`).

### 1.3 Zero-cycle register moves
- The following register-to-register moves execute with zero cycle delay: `MOV r32/r64, r32/r64`, `MOVSXD r32, r32`, `XCHG EAX/RAX, r32/r64`, `XCHG r32/r64, r32/r64`, `(V)MOVAP(D/S)` between SIMD registers, and `(V)MOVDQ(U/A)` between SIMD registers (`57647_zen4_sog.txt:1278` `57647_zen4_sog.txt:1281` `57647_zen4_sog.txt:1283` `57647_zen4_sog.txt:1285` `57647_zen4_sog.txt:1287` `57647_zen4_sog.txt:1289` `57647_zen4_sog.txt:1291`).

### 1.4 Stack pointer tracking (rSP)
- The integer rename unit can track implicit stack pointer updates so later instructions no longer depend on older rSP-modifying uops (`57647_zen4_sog.txt:1295` `57647_zen4_sog.txt:1298`).
- Tracking covers `PUSH` (except `PUSH rSP`), `POP` (except `POP rSP`), `CALL` near rel/abs and `RET` near/near imm (`57647_zen4_sog.txt:1302` `57647_zen4_sog.txt:1304` `57647_zen4_sog.txt:1306` `57647_zen4_sog.txt:1308` `57647_zen4_sog.txt:1310` `57647_zen4_sog.txt:1312`).
- Loads/stores that use rSP as base or index, `MOV reg, rSP`, and `LEA` with `[rSP + disp]`, `[rSP]`, or `[rSP + index x scale + disp]` consume the tracked value without re-introducing dependencies (`57647_zen4_sog.txt:1316` `57647_zen4_sog.txt:1318` `57647_zen4_sog.txt:1320` `57647_zen4_sog.txt:1321`).
- Any rSP update or use outside those lists causes an extra uop and resets tracking until a supported update occurs again (`57647_zen4_sog.txt:1323` `57647_zen4_sog.txt:1325`).

### 1.5 RIP access and branch fusion
- Instead of `CALL 0`, 64-bit code can read the instruction pointer into a GPR via `LEA` with RIP-relative addressing, e.g., `LEA RAX, [RIP+0]` (`57647_zen4_sog.txt:916` `57647_zen4_sog.txt:918`).
- The decode unit fuses a conditional branch with a preceding flag-writing instruction (CMP/TEST/SUB/ADD/INC/DEC/OR/AND/XOR) when the branch immediately follows, eliminating one macro-op (`57647_zen4_sog.txt:1208` `57647_zen4_sog.txt:1216` `57647_zen4_sog.txt:1229`).
- Fusion is disallowed for JCXZ, for flag writers containing both an immediate and a displacement, and for RIP-relative flag writers; the combined pair must be 15 bytes or less (`57647_zen4_sog.txt:1235` `57647_zen4_sog.txt:1237` `57647_zen4_sog.txt:1239` `57647_zen4_sog.txt:1241`).
- When CMP mixes a register with a memory operand, place memory second (opcodes 0x3A/0x3B) to retain peak throughput (`57647_zen4_sog.txt:1243` `57647_zen4_sog.txt:1244`).

### 1.6 DIV/IDIV fusion
- Zen4 fuses the common sequences `XOR rDX, rDX` + `DIV reg` and `CDQ/CQO` + `IDIV reg`, removing one macro-op if the register operand is not rDX and the operand sizes match (`57647_zen4_sog.txt:1248` `57647_zen4_sog.txt:1252` `57647_zen4_sog.txt:1254` `57647_zen4_sog.txt:1256` `57647_zen4_sog.txt:1257` `57647_zen4_sog.txt:1260`). Memory operands do not participate.

### 1.7 Other GPR considerations
- Integer multiply instructions that write two destination registers (e.g., hi/lo products in rDX:rAX) incur one extra cycle of latency for the second result (`57647_zen4_sog.txt:1422` `57647_zen4_sog.txt:1423`).
- A flat memory model (64-bit mode or 32-bit with `CS.Base = 0` and `CS.Limit = FFFFFFFFh`) is required to leverage the Op Cache and its register-dependent optimizations (`57647_zen4_sog.txt:1162` `57647_zen4_sog.txt:1163`).

### 1.8 Load latency adjustments by destination type
- The latency tables reference register-to-register forms; loads from memory must add cache-hit penalties depending on the destination register file (`57647_zen4_sog.txt:2111` `57647_zen4_sog.txt:2112`).
- For GPR destinations add 4 cycles for an L1D hit, 14 for L2, and roughly 50 for L3 (`57647_zen4_sog.txt:2117` `57647_zen4_sog.txt:2119` `57647_zen4_sog.txt:2121` `57647_zen4_sog.txt:2123`).
- For FP/SIMD destinations add 7 cycles for L1D, 17 for L2, and about 53 for L3 (`57647_zen4_sog.txt:2124` `57647_zen4_sog.txt:2126` `57647_zen4_sog.txt:2128` `57647_zen4_sog.txt:2130`).
- Complex addressing (scaled index), non-zero segment bases, misaligned operands, and 512-bit loads each add another cycle, and AVX-512 merge-masked loads can further raise latency or lower throughput (`57647_zen4_sog.txt:2132` `57647_zen4_sog.txt:2135` `57647_zen4_sog.txt:2137` `57647_zen4_sog.txt:2138`).

## 2. Vector and floating-point registers

### 2.1 FPU organization
- The floating-point unit operates as a coprocessor for X87, MMX, XMM, YMM, ZMM, and FP control/status registers, maintaining its own scheduler, register file, and renamer separate from the integer side (`57647_zen4_sog.txt:1461` `57647_zen4_sog.txt:1462` `57647_zen4_sog.txt:1463`).
- The FP scheduler can dispatch six macro-ops per cycle, issues one micro-op per pipe, tracks 2x32 macro-ops, and overflows into a 64-entry Non-Scheduling Queue for address-calculation acceleration (`57647_zen4_sog.txt:1464` `57647_zen4_sog.txt:1465` `57647_zen4_sog.txt:1468`).
- The unit accepts up to two 256-bit loads per cycle and includes dedicated buses to move data between FP registers and the general-purpose register domain; two store-data pipelines serve FP stores and FP→GPR transfers (`57647_zen4_sog.txt:1472` `57647_zen4_sog.txt:1473` `57647_zen4_sog.txt:1474` `57647_zen4_sog.txt:1475`).
- AVX-512 is supported with 512-bit storage in the FP register file; 512-bit ops issue over two cycles because the datapaths are 256 bits wide, lowering queue pressure without sacrificing peak FLOPS (`57647_zen4_sog.txt:1478` `57647_zen4_sog.txt:1479`).

### 2.2 Register width usage
- Best performance-per-watt is achieved when code consistently operates on full-width 256-bit YMM or 512-bit ZMM registers, because wider instructions reduce macro-op overhead and power draw (`57647_zen4_sog.txt:1519` `57647_zen4_sog.txt:1520` `57647_zen4_sog.txt:1522`).
- Load and store entire registers in one instruction whenever possible (`vmovapd` instead of `movapd`/`movlpd`/`movhpd`); if multiple loads are unavoidable, place them back to back (`57647_zen4_sog.txt:1523` `57647_zen4_sog.txt:1524` `57647_zen4_sog.txt:1525`).
- The STORE pipeline in Table 2 serves both memory stores and moves into the EX (GPR) domain, so heavy FP→GPR traffic competes with stores for those scheduler slots (`57647_zen4_sog.txt:1558` `57647_zen4_sog.txt:1561`).

### 2.3 Managing FP/SIMD register contents
- Clear FP registers with the zeroing idioms once results are consumed so their physical entries can be reused for speculation and to avoid merge dependencies on partially updated registers (`57647_zen4_sog.txt:1597` `57647_zen4_sog.txt:1599`).
- `XMM/YMM/ZMM` register-to-register moves have zero latency and can be used freely for permutes or data motion (`57647_zen4_sog.txt:1610` `57647_zen4_sog.txt:1611`).
- Prefer the register-destination forms of COMPRESS instructions; the memory-destination forms are implemented in microcode and cap store bandwidth (`57647_zen4_sog.txt:1617` `57647_zen4_sog.txt:1618` `57647_zen4_sog.txt:1619`).
- In x87 code, `FXCH` is far faster than push/pop for swapping register stack entries, and nothing should intervene between `FCOM` and `FSTSW` to keep comparisons precise (`57647_zen4_sog.txt:1622` `57647_zen4_sog.txt:1623`).

### 2.4 MXCSR, denormals, and x87 FCW
- Set `MXCSR.DAZ` (Denormals Are Zero) and `MXCSR.FTZ` (Flush To Zero) when denormal precision is not required to prevent multiply/divide/sqrt replays when denormals arise (`57647_zen4_sog.txt:1601` `57647_zen4_sog.txt:1602` `57647_zen4_sog.txt:1605`).
- Denormal penalties depend on MXCSR configuration and instruction sequences; enabling both `DAZ` and `FTZ` avoids the penalties but deviates from IEEE-754 behavior (`57647_zen4_sog.txt:1629` `57647_zen4_sog.txt:1631` `57647_zen4_sog.txt:1632`).
- The x87 Floating-Point Control Word lacks equivalents of DAZ/FTZ, so denormal penalties cannot be mitigated for x87 operations (`57647_zen4_sog.txt:1642` `57647_zen4_sog.txt:1643`).

### 2.5 SIMD zeroing and ones idioms
- SIMD zeroing idioms include `VXORP(S/D)`, `VANDNP(S/D)`, `VPCMPGT(B/W/D/Q)`, `VPANDN`, `VPXOR`, and `VPSUB(B/W/D/Q)`, all clearing the destination register in zero cycles (`57647_zen4_sog.txt:1188` `57647_zen4_sog.txt:1190` `57647_zen4_sog.txt:1192` `57647_zen4_sog.txt:1194` `57647_zen4_sog.txt:1196` `57647_zen4_sog.txt:1198`).
- Ones idioms fill the destination with all ones and break dependencies: `PCMPEQ(B/W/D/Q)` for XMM and `VPCMPEQ(B/W/D/Q)` for ZMM/YMM/XMM registers (`57647_zen4_sog.txt:1203` `57647_zen4_sog.txt:1205`).

### 2.6 XMM register merge and SSE/AVX mixing
- Zen4 tracks XMM registers whose upper lanes have been zeroed so scalar instructions can bypass merge operations on those upper bits, e.g., allowing `SQRTSS` to proceed without waiting for prior writers when the upper 96 bits stay zero (`57647_zen4_sog.txt:1647` `57647_zen4_sog.txt:1649` `57647_zen4_sog.txt:1651` `57647_zen4_sog.txt:1653`).
- The merge optimization applies to `CVTPI2PS`, `CVTSI2SS` (32/64-bit), `MOVSS xmm1,xmm2`, `CVTSD2SS`, `CVTSS2SD`, `MOVLPS xmm1,[mem]`, `CVTSI2SD` (32/64-bit), `MOVSD xmm1,xmm2`, `MOVLPD xmm1,[mem]`, `RCPSS`, `ROUNDSS`, `ROUNDSD`, `RSQRTSS`, `SQRTSD`, and `SQRTSS` (`57647_zen4_sog.txt:1657` `57647_zen4_sog.txt:1659` `57647_zen4_sog.txt:1661` `57647_zen4_sog.txt:1663` `57647_zen4_sog.txt:1665` `57647_zen4_sog.txt:1667` `57647_zen4_sog.txt:1669` `57647_zen4_sog.txt:1671` `57647_zen4_sog.txt:1673` `57647_zen4_sog.txt:1675` `57647_zen4_sog.txt:1677` `57647_zen4_sog.txt:1679` `57647_zen4_sog.txt:1681` `57647_zen4_sog.txt:1683` `57647_zen4_sog.txt:1685`).
- Mixing SSE and AVX instructions incurs a penalty whenever the upper 384 bits of the ZMM registers hold non-zero data; SSE ops are internally promoted to 256/512 bits to merge the upper lanes, creating extra dependencies (`57647_zen4_sog.txt:1688` `57647_zen4_sog.txt:1689` `57647_zen4_sog.txt:1690`).

## 3. Physical register files and resource sharing

- The retire unit manages integer register mapping and renaming; the integer physical register file contains 224 entries, up to 38 per thread mapped to architectural or temporary state with the remainder available for renames (`57647_zen4_sog.txt:1449` `57647_zen4_sog.txt:1450` `57647_zen4_sog.txt:1451`).
- Flags are stored in a dedicated physical register file that provides roughly 108 free entries for renaming flag-writing instructions, separate from the integer PRF (`57647_zen4_sog.txt:1452` `57647_zen4_sog.txt:1453`).
- Under SMT, the resource-sharing table shows that the integer register file and floating-point physical register file are competitively shared between threads, as are other queues such as the write-combining buffer, so register pressure in one thread directly impacts the other (`57647_zen4_sog.txt:2021` `57647_zen4_sog.txt:2025` `57647_zen4_sog.txt:2035`).

## 4. SIMD, mask, and operand notation

- Instruction tables use `mmx` for any 64-bit MMX register, `reg` for any general-purpose register, `regN` for an N-bit GPR, `xmmN`/`ymmN`/`zmmN` to distinguish multiple operands of the same SIMD size, and `k` for any AVX-512 mask register (`57647_zen4_sog.txt:2175` `57647_zen4_sog.txt:2178` `57647_zen4_sog.txt:2179` `57647_zen4_sog.txt:2180` `57647_zen4_sog.txt:2182` `57647_zen4_sog.txt:2184` `57647_zen4_sog.txt:2186`).
- `{k1}{z}` denotes AVX-512 zero masking, while `{k1}` alone denotes merge masking, highlighting that mask registers control whether upper lanes are zeroed or preserved on many vector operations (`57647_zen4_sog.txt:2187` `57647_zen4_sog.txt:2188`).

## 5. System and MSR-related registers

### 5.1 PKRU, privileged reads, and shadow stack state
- Zen4 supports RDPRU along with RDPKRU/WRPKRU, enabling user-mode code to read privilege registers and to read/write the PKRU protection-key register where the ISA allows (`57647_zen4_sog.txt:321` `57647_zen4_sog.txt:333` `57647_zen4_sog.txt:335`).
- Control-flow enforcement/shadow stack instructions manipulate the shadow stack pointer (SSP) registers: `INCSSP`, `RDSSP`, `SAVEPREVSSP`, `RSTORSSP`, `WRSS`, `WRUSS`, `SETSSBSY`, and `CLRSSBSY` are all supported (`57647_zen4_sog.txt:359` `57647_zen4_sog.txt:361` `57647_zen4_sog.txt:363` `57647_zen4_sog.txt:365` `57647_zen4_sog.txt:367` `57647_zen4_sog.txt:369` `57647_zen4_sog.txt:371` `57647_zen4_sog.txt:373`).

### 5.2 Prefetch control and cache-type registers
- Some server models implement a Prefetch Control MSR that can individually disable or enable each prefetcher; CPUID enumerates the capability and the PPR documents the MSR fields (`57647_zen4_sog.txt:1799` `57647_zen4_sog.txt:1800` `57647_zen4_sog.txt:1801`).
- Zen4 supports MTRR and PAT so software can program ranges as WB, WP, WT, UC, or WC; defining WC ranges lets the core merge writes into the write-combining buffers (`57647_zen4_sog.txt:1804` `57647_zen4_sog.txt:1805` `57647_zen4_sog.txt:1813` `57647_zen4_sog.txt:1814` `57647_zen4_sog.txt:1815`).
- The document reiterates the relevant acronyms (MTRR, PAT, UC, WC, WT, WP, WB) for these memory-type registers (`57647_zen4_sog.txt:1829` `57647_zen4_sog.txt:1830` `57647_zen4_sog.txt:1832` `57647_zen4_sog.txt:1834` `57647_zen4_sog.txt:1836` `57647_zen4_sog.txt:1838`).
- Write-combining regions must be configured via MTRR/PAT, and the official AMD64 System Programming docs plus the Family 19h PPR carry the exact register definitions (`57647_zen4_sog.txt:1868` `57647_zen4_sog.txt:1869`).
- Streaming stores such as `MOVNTQ`/`MOVNTI` use the write buffers and behave like WC regions managed by MTRR/PAT, inheriting their flush rules (`57647_zen4_sog.txt:1887` `57647_zen4_sog.txt:1889` `57647_zen4_sog.txt:1890` `57647_zen4_sog.txt:1891`).

### 5.3 Events that close the write-combining buffer
- IN/INS/OUT/OUTS treat memory as UC and therefore close the write-combining buffer (`57647_zen4_sog.txt:1929` `57647_zen4_sog.txt:1931`).
- Serializing instructions such as `MOVCRx`, `MOVDRx`, `WRMSR`, `INVD`, `INVLPG`, `WBINVD`, `LGDT`, `LLDT`, `LIDT`, `LTR`, `CPUID`, `IRET`, `RSM`, `INIT`, and `HALT` also flush the buffer, tying MSR, control-register, and descriptor-table accesses directly to store visibility (`57647_zen4_sog.txt:1932` `57647_zen4_sog.txt:1934` `57647_zen4_sog.txt:1935`).
- `CLFLUSH` only closes the WCB when the target memory type is WC or UC (`57647_zen4_sog.txt:1936` `57647_zen4_sog.txt:1937`), and any cache/bus lock closes the buffers before the locked sequence begins (`57647_zen4_sog.txt:1938` `57647_zen4_sog.txt:1939`).

### 5.4 LOCK guidance and debug MSRs
- To benefit from Zen4's LOCK optimizations, keep locked accesses within 16-byte alignment, postpone FP instructions after a lock, and ensure the Last Branch Record facility is disabled by clearing `DBG_CTL_MSR.LBR` (`57647_zen4_sog.txt:2056` `57647_zen4_sog.txt:2058` `57647_zen4_sog.txt:2061`).
