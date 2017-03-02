# igmnes
(yet another) NES emulator written in Rust

Project status:
| Component     | Status                                                            |
| :------------:|:------------------------------------------------------------------|
| CPU           | Fully functional 6502 implementation (barring unofficial opcodes) |     
| APU           | Partially implemented (Pulse channels only at the moment)         |
| PPU           | Not implemented                                                   |  
| Input         | Not implemented                                                   |
| Mappers       | Mapper 000 (NROM) only                                            |
| Debugger      | Terminal CPU debugger implemented                                 |

To-do list:
| Component     | To-Do
| ------------- | ----------- |
| General       | Implement NMI and IRQ handling between components             |
| CPU           | Implement unofficial opcodes                                  |     
| APU           | Implement Triangle, Noise and DMC channels                    |
| PPU           | Everything                                                    |
| Input         | Everything                                                    |
| Mappers       | Implement (at the very least) more popular mappers            |
| Debugger      | Implement a visual debugger frontend; Implement PPU debugging |
