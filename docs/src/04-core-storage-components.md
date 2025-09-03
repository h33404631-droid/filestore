# Core Storage Components

This chapter covers the key storage components that work alongside the Record Manager to provide a complete storage solution.

## Memory Pool Manager

**Memory Layout Strategy**:
```
Virtual Address Space Allocation:
├── Data Regions (Multiple files)
│   ├── Primary Data File (records.dat)
│   ├── Overflow Data File (overflow.dat)
│   └── Archive Data File (archive.dat)
├── Index Regions (Multiple files)
│   ├── Hash Indexes (hash_*.idx)
│   ├── Range Indexes (range_*.idx)
│   └── Composite Indexes (comp_*.idx)
└── Control Regions
    ├── Metadata (metadata.dat)
    ├── WAL (wal.dat)
    └── Free Space Maps (freemap.dat)
```

**Memory Mapping Configuration**:
- **File Pre-allocation**: Create large files and map entire regions
- **Huge Page Support**: Use 2MB/1GB pages to reduce TLB pressure
- **NUMA Optimization**: Allocate on local NUMA nodes
- **Page Fault Minimization**: Pre-fault critical pages during startup

## Index Manager

**Index Types Supported**:

**Hash Indexes**:
- Primary key lookups (O(1) average case)
- Composite key lookups
- Configurable hash functions and collision handling

**Range Indexes**:
- Sorted access patterns (B+ tree style in memory)
- Range queries and scans
- Multi-column sorting support

**Bitmap Indexes**:
- Categorical data with low cardinality
- Fast set operations (AND, OR, NOT)
- Compressed representation

**Index File Structure**:
```
Index File Layout:
├── Index Header (metadata, statistics)
├── Index Structure (hash table/tree nodes)
├── Key Storage (variable length keys)
└── Overflow Areas (collision handling)
```

## Write-Ahead Log (WAL) System

**WAL Architecture**:
```
WAL Components:
├── In-Memory Write Buffer (circular buffer)
├── Background Flush Thread
├── WAL File Management (rotation, cleanup)
└── Recovery Coordinator
```

**WAL Entry Structure**:
- Transaction ID and sequence number
- Operation type (INSERT/UPDATE/DELETE)
- Before/after images for updates
- Checksum for integrity verification
- Timestamp for recovery ordering

**Flush Strategies**:
- **Immediate**: Sync after each transaction (highest durability)
- **Batched**: Group commits for higher throughput
- **Async**: Background flushing for maximum performance
- **Configurable**: Per-table or per-operation settings

## Transaction Management

**Concurrency Control**:
- **MVCC (Multi-Version Concurrency Control)**: Support concurrent readers/writers
- **Optimistic Locking**: Detect conflicts at commit time
- **Lock-Free Reads**: Readers don't block writers
- **Configurable Isolation**: Support different isolation levels

**Transaction Lifecycle**:
```
Transaction Flow:
1. Begin → Acquire transaction ID
2. Operations → Log to private buffer
3. Commit → Validate + Write to WAL
4. Complete → Update indexes + notify
5. Cleanup → Release resources
```
