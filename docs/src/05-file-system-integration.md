# File System Integration

## File Organization

**Directory Structure**:
```
storage_root/
├── data/
│   ├── records.dat          # Primary record storage
│   └── overflow.dat         # Overflow record storage
├── indexes/
│   ├── primary.idx          # Primary key index
│   ├── user.idx            # User-based index
│   ├── timestamp.idx       # Time-based index
│   └── composite.idx       # Multi-field indexes
├── wal/
│   ├── current.wal
│   └── archived/
├── snapshots/
└── metadata/
    ├── schema.meta
    └── config.meta
```

**File Sizing Strategy**:
- **Initial Size**: Pre-allocate files based on capacity planning
- **Growth Strategy**: Exponential growth with configurable limits
- **Compaction**: Background compaction to reclaim space
- **Archival**: Move old data to cost-optimized storage

## Storage Tiering

**Multi-Tier Storage Architecture**:
```
Storage Tiers:
├── Hot Tier (Local NVMe) - UNCOMPRESSED
│   ├── Active records and indexes (raw binary)
│   ├── Current WAL files (minimal encoding)
│   └── Frequently accessed data (cache-optimized)
├── Warm Tier (Network Storage) - MINIMAL COMPRESSION
│   ├── Recent snapshots (light compression)
│   ├── Archived WAL files (batched encoding)
│   └── Infrequently accessed records (selective compression)
└── Cold Tier (Object Storage) - HEAVY COMPRESSION
    ├── Long-term backups (ZSTD compression)
    ├── Historical snapshots (archive formats)
    └── Compliance archives (maximum compression ratio)
```

**Compression Strategy by Tier**:
- **Hot Tier**: No compression - prioritize speed over space
- **Warm Tier**: Optional lightweight compression for cost savings
- **Cold Tier**: Aggressive compression using ZSTD/LZMA for maximum space efficiency
