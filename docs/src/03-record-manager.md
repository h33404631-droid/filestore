# Table Manager

## Table of Contents

- [Core Concepts](#core-concepts)
  - [What is a Table?](#what-is-a-table)
  - [What are Records?](#what-are-records)
- [Table Manager Architecture](#table-manager-architecture)
- [Table Organization](#table-organization)
  - [Table Structure](#table-structure)
  - [Record Layout](#record-layout)
- [Record Operations](#record-operations)
  - [Storage and Addressing](#storage-and-addressing)
  - [Record IDs (Snowflake Format)](#record-ids-snowflake-format)
  - [Record Lifecycle](#record-lifecycle)
- [Index System](#index-system)
  - [Primary Index](#primary-index)
  - [Secondary Indexes](#secondary-indexes)
  - [Index Storage](#index-storage)
  - [Query Processing](#query-processing)
- [Advanced Features](#advanced-features)
  - [Multi-Version Concurrency (MVCC)](#multi-version-concurrency-mvcc)
  - [Schema Management](#schema-management)
  - [Zero-Copy Storage](#zero-copy-storage)
- [Performance](#performance)
  - [Data Types and Storage](#data-types-and-storage)
  - [Performance Comparison](#performance-comparison)
  - [Optimization Strategies](#optimization-strategies)
- [Page-Aware Storage Design](#page-aware-storage-design)
  - [Read/Write Amplification Problem](#readwrite-amplification-problem)
  - [Page-Aligned Record Layout](#page-aligned-record-layout)
  - [Batch Operations](#batch-operations)
  - [Hot Page Management](#hot-page-management)
- [Data Integrity](#data-integrity)
- [Catalog System](#catalog-system)
  - [Catalog Structure](#catalog-structure)
  - [Catalog Operations](#catalog-operations)
  - [Catalog Persistence](#catalog-persistence)

## Core Concepts

### What is a Table?

A **table** is the primary organization unit in this storage engine. It's a specialized container that stores one type of business data with optimal performance.

**Key Characteristics**:
- **Single Data Type**: Each table stores only one kind of business object (orders, users, transactions)
- **Fixed Record Size**: All records in a table have the same size, determined at table creation
- **Memory-Mapped**: Direct access to data without serialization overhead
- **High Performance**: Optimized for sub-millisecond access times

**Table Examples**:
- **Orders Table**: 512 bytes per record - trading orders with symbol, quantity, price
- **Users Table**: 256 bytes per record - user profiles and account information  
- **Logs Table**: 128 bytes per record - system events and audit trails
- **Config Table**: 1024 bytes per record - complex configuration settings

### What are Records?

**Records** are the individual data entries stored within tables. Each record represents one business entity.

**Record Characteristics**:
- **Structured Data**: Fixed schema with typed fields (like a programming struct)
- **Unique Identity**: Every record has a Snowflake ID for direct access
- **Binary Storage**: Data stored in CPU-native format for maximum speed
- **Atomic Operations**: Read or write entire records without corruption

**Why This Design Works**:
- **vs Traditional Database**: No SQL parsing, direct memory access
- **vs JSON/XML**: Binary format is 100x faster than text parsing
- **vs Key-Value Store**: Structured schema enables efficient queries
- **vs Document Store**: Fixed size eliminates memory fragmentation

**Performance Advantage**:
```
Traditional Database:
Request → Parse SQL → Plan Query → Buffer Pool → Disk I/O → Response
         ↑ 5-20ms of overhead ↑

Table Manager:
Request → Table Lookup → Direct Memory Access → Response  
         ↑ <1ms total time ↑
```

## Table Manager Architecture

The Table Manager is the central component that handles all table and record operations. It replaces the traditional "Record Manager" concept with a more focused, table-centric approach.

**Core Design Principle**:
Instead of managing records independently, the Table Manager treats each table as a self-contained unit that knows how to manage its own records efficiently.

**Architecture Components**:
```
Table Manager
├── Table Registry          # Active tables lookup
├── Storage Engine         # Memory mapping and I/O
├── Transaction Manager    # MVCC and concurrent access  
├── Catalog Service       # Schema and metadata
└── Index Manager         # Secondary indexes
```

**Table Manager Responsibilities**:
- **Table Lifecycle**: Create, drop, and manage table instances
- **Record Operations**: Insert, update, delete, query records within tables
- **Schema Management**: Validate records against table schemas
- **Memory Management**: Allocate and track record slots within tables
- **Concurrency Control**: Coordinate MVCC versions and transactions
- **Performance Optimization**: Cache hot data, batch operations

**Key Benefits of Table-Centric Design**:
- **Simplified Architecture**: Fewer abstraction layers, clearer responsibilities
- **Better Performance**: Direct table → storage mapping
- **Type Safety**: Each table enforces its record schema
- **Locality**: Related records (same type) stored together
- **Scalability**: Tables can be independently optimized and managed

## Table Organization

### Table Structure

Each table is a self-contained storage unit optimized for a specific record type.

**Table Design Principles**:
- **Homogeneous Records**: All records in a table have identical structure and size
- **Contiguous Storage**: Records stored sequentially in memory-mapped files
- **Fixed Allocation**: Record slots pre-allocated for predictable performance
- **Type Safety**: Schema enforced at the table level

**Table Components**:
```
Table Structure:
┌─────────────────────────────────────────────┐
│ Table Header (metadata, schema, stats)     │
├─────────────────────────────────────────────┤
│ Allocation Bitmap (track free/used slots)  │  
├─────────────────────────────────────────────┤
│ Record Slot 0                              │
├─────────────────────────────────────────────┤
│ Record Slot 1                              │
├─────────────────────────────────────────────┤
│ ...                                        │
├─────────────────────────────────────────────┤
│ Record Slot N                              │
└─────────────────────────────────────────────┘
```

### Record Layout

Every record within a table follows the same binary layout:

```
Record Structure (size varies by table):
┌─────────────────────────────────────────────┐
│ Record Header (32 bytes)                   │  ← Metadata
├─────────────────────────────────────────────┤
│ Application Data (table-specific size)     │  ← Your data
├─────────────────────────────────────────────┤
│ Record Footer (32 bytes)                   │  ← Integrity
└─────────────────────────────────────────────┘
```

**Record Header (32 bytes)**:
```
Header Layout:
├── Record ID (8 bytes)           # Unique Snowflake ID
├── Schema Version (4 bytes)      # Handle schema evolution  
├── Data Length (4 bytes)         # Actual payload size
├── Status Flags (4 bytes)        # ACTIVE, DELETED, etc.
├── Timestamp (8 bytes)           # Last modification time
├── Header Checksum (4 bytes)     # Header integrity check
```

**Table Size Examples (Page-Aware Design)**:
```
Table Type    | Record Size | Records/Page | Max Records | Memory Usage | Page Efficiency
--------------|-------------|--------------|-------------|--------------|----------------
Users         | 256B        | 16 records   | 4M records  | 1GB          | 100% (4096/256)
Orders        | 512B        | 8 records    | 2M records  | 1GB          | 100% (4096/512)  
Logs          | 128B        | 32 records   | 8M records  | 1GB          | 100% (4096/128)
Config        | 1024B       | 4 records    | 1M records  | 1GB          | 100% (4096/1024)
```

## Record Operations

### Storage and Addressing

Each table manages its own record storage independently.

**Table Memory Organization**:
- **Dedicated Memory Pool**: Each table has its own memory-mapped region
- **Slot-Based Allocation**: Records occupy fixed-size slots within the table
- **Bitmap Tracking**: Fast free/used slot tracking using bitmaps
- **Sequential Layout**: Records stored contiguously for cache efficiency

**Record Access Pattern**:
```
Operation Flow:
1. Application specifies table name + record ID
2. Table Manager looks up table instance
3. Use Primary Index: record_id → slot_position
4. Direct memory access to slot
5. Return record data
```

**Index-Based Address Calculation**:
```rust
fn get_record_address(table: &Table, record_id: RecordId) -> Option<*mut u8> {
    // Use primary index to find slot position
    let slot_index = table.primary_index.get(record_id)?;
    let offset = slot_index * table.record_size;
    Some(table.memory_pool.base_addr + offset)
}
```

**Performance Characteristics**:
- **O(1) Access**: Direct memory addressing
- **No Fragmentation**: Fixed-size slots eliminate fragmentation
- **Cache Friendly**: Sequential record layout improves cache hits
- **NUMA Aware**: Tables can be allocated on specific NUMA nodes

**Record Status Flags**:
The 4-byte status flags field tracks various record states:
```
Status Flags Breakdown:
├── Bits 0-3: Lifecycle State (ACTIVE, DELETED, TOMBSTONE)
├── Bits 4-7: Lock State (UNLOCKED, READ_LOCK, WRITE_LOCK)
├── Bits 8-11: Encoding Format (BINARY, JSON, etc.)
├── Bits 12-15: Validation State (VALID, CORRUPT, etc.)
├── Bits 16-31: Reserved for extensions
```

### Record IDs (Snowflake Format)

Every record across all tables uses the same ID format for consistency and performance.

**Snowflake ID Structure (64 bits)**:
```
Record ID Components:
├── Timestamp (41 bits)          # Creation time in milliseconds
├── Datacenter ID (5 bits)       # Datacenter identifier (0-31)
├── Machine ID (5 bits)          # Machine identifier (0-31)  
├── Sequence Number (12 bits)    # Per-machine counter (0-4095)
├── Sign Bit (1 bit)            # Always 0 (positive)
```

**ID Generation Properties**:
- **Globally Unique**: No coordination needed between machines
- **Time-Ordered**: IDs roughly ordered by creation time
- **High Throughput**: 4096 IDs per millisecond per machine
- **Distributed**: No single point of failure
- **Debuggable**: Can extract timestamp and source machine

**Record Lookup Process**:
```rust
// Table Manager uses index-based lookup
fn find_record(table_name: &str, record_id: RecordId) -> Option<&Record> {
    let table = self.get_table(table_name)?;
    // Primary index maps record_id to slot_position
    let slot_index = table.primary_index.get(record_id)?;
    table.get_record_at_slot(slot_index)
}
```

**Key Properties for Index System**:
- **Globally Unique**: Each record ID is unique across entire system
- **Random Distribution**: No correlation between ID and storage location  
- **Index-Friendly**: IDs work efficiently in hash tables and B+ trees
- **Cross-Table Queries**: Consistent ID format enables joins and references
- **Debugging**: Can extract creation timestamp and source information

### Record Lifecycle

Each table manages the lifecycle of its records independently.

**Record State Machine**:
```
Record States within Table:
    ACTIVE ──delete──→ DELETED ──expire──→ TOMBSTONE
       ↑                  │                    │
       └──────resurrect───┘                    │
                                               │
                           ←──────cleanup──────┘
```

**State Definitions**:
- **ACTIVE**: Record is live and available for operations
- **DELETED**: Soft-deleted, still recoverable, counts toward table capacity
- **TOMBSTONE**: Marked for cleanup, space will be reclaimed

**Table-Level Operations**:

**Insert Record**:
```rust
impl Table {
    fn insert(&mut self, data: &[u8]) -> Result<RecordId> {
        let slot = self.find_free_slot()?;
        let record_id = self.generate_snowflake_id();
        
        // Write record to slot
        self.write_record(slot, record_id, data)?;
        self.mark_slot_used(slot);
        
        // Update primary index: record_id → slot_position
        self.primary_index.insert(record_id, slot)?;
        
        // Update secondary indexes
        self.update_secondary_indexes(record_id, data)?;
        
        Ok(record_id)
    }
}
```

**Read Record**:
```rust
fn get(&self, record_id: RecordId) -> Option<&Record> {
    // Use primary index to find slot
    let slot = self.primary_index.get(record_id)?;
    let record = self.get_slot(slot)?;
    if record.is_active() {
        Some(record)
    } else {
        None
    }
}
```

**Update Record (MVCC)**:
```rust
fn update(&mut self, record_id: RecordId, data: &[u8]) -> Result<()> {
    // Find current record location
    let slot = self.primary_index.get(record_id)?;
    
    // Create new version while preserving old for concurrent readers
    self.create_new_version(record_id, data)?;
    self.update_version_chain(record_id)?;
    
    // Update secondary indexes with new data
    self.update_secondary_indexes(record_id, data)?;
    
    Ok(())
}
```

**Delete Record**:
```rust
fn delete(&mut self, record_id: RecordId) -> Result<()> {
    // Find record location via primary index
    let slot = self.primary_index.get(record_id)?;
    
    // Mark record as deleted (soft delete)
    self.mark_record_deleted(slot);
    
    // Remove from secondary indexes
    self.remove_from_secondary_indexes(record_id)?;
    
    // Keep primary index entry for recovery
    // Data still exists for recovery
    Ok(())
}
```

## Index System

The index system is crucial for efficient record lookup and query processing. Instead of directly mapping record IDs to slot positions, we use a flexible index architecture.

### Primary Index

**Purpose**: Maps Record ID (Snowflake) → Slot Position in the table

**Design Rationale**:
- Record IDs are globally unique random numbers (Snowflake format)
- No correlation between Record ID and physical storage location
- Primary index provides O(1) lookup from ID to slot position
- Enables flexible slot allocation and compaction

**Primary Index Structure**:
```rust
struct PrimaryIndex {
    // Hash table for O(1) lookup
    index: HashMap<RecordId, SlotIndex>,
    
    // Optional: B+ tree for range queries on IDs
    btree: Option<BTreeMap<RecordId, SlotIndex>>,
    
    // Metadata
    table_id: TableId,
    record_count: u64,
}

impl PrimaryIndex {
    fn get(&self, record_id: RecordId) -> Option<SlotIndex> {
        self.index.get(&record_id).copied()
    }
    
    fn insert(&mut self, record_id: RecordId, slot: SlotIndex) -> Result<()> {
        if self.index.contains_key(&record_id) {
            return Err(Error::DuplicateKey);
        }
        self.index.insert(record_id, slot);
        self.record_count += 1;
        Ok(())
    }
    
    fn remove(&mut self, record_id: RecordId) -> Option<SlotIndex> {
        let slot = self.index.remove(&record_id)?;
        self.record_count -= 1;
        Some(slot)
    }
}
```

**Storage Characteristics**:
- **In-Memory**: Primary index kept entirely in RAM for speed
- **Persistent**: Periodically flushed to disk for crash recovery
- **Memory Overhead**: ~24 bytes per record (16-byte ID + 8-byte slot + overhead)
- **Performance**: O(1) lookup, O(1) insert/delete

### Secondary Indexes

**Purpose**: Enable efficient queries on non-primary key fields

**Index Types Supported**:

**Hash Indexes** (for equality queries):
```rust
struct HashSecondaryIndex {
    name: String,
    field: FieldName,
    index: HashMap<FieldValue, Vec<RecordId>>,  // Multiple records per value
}
```

**B+ Tree Indexes** (for range queries):
```rust
struct BTreeSecondaryIndex {
    name: String,
    field: FieldName,
    index: BTreeMap<FieldValue, Vec<RecordId>>,
}
```

**Composite Indexes** (multi-field):
```rust
struct CompositeIndex {
    name: String,
    fields: Vec<FieldName>,
    index: BTreeMap<CompositeKey, Vec<RecordId>>,
}
```

**Secondary Index Operations**:
```rust
impl SecondaryIndex {
    // Find records by field value
    fn find(&self, value: &FieldValue) -> Vec<RecordId> {
        self.index.get(value).cloned().unwrap_or_default()
    }
    
    // Range query (for B+ tree indexes)
    fn range(&self, start: &FieldValue, end: &FieldValue) -> Vec<RecordId> {
        self.index
            .range(start..=end)
            .flat_map(|(_, record_ids)| record_ids.iter().copied())
            .collect()
    }
    
    // Update index when record changes
    fn update(&mut self, record_id: RecordId, old_value: Option<&FieldValue>, new_value: Option<&FieldValue>) -> Result<()> {
        // Remove old entry
        if let Some(old_val) = old_value {
            self.remove_entry(old_val, record_id);
        }
        
        // Add new entry  
        if let Some(new_val) = new_value {
            self.add_entry(new_val.clone(), record_id);
        }
        
        Ok(())
    }
}
```

### Index Storage

**Index File Organization**:
```
Table Directory Structure:
├── data/
│   └── table_records.dat        # Record data (mmap file)
├── indexes/
│   ├── primary.idx             # Primary index (RecordID → Slot)
│   ├── user_id.idx             # Secondary index on user_id
│   ├── timestamp.idx           # Secondary index on timestamp
│   └── symbol_price.idx        # Composite index
└── metadata/
    ├── table.meta              # Table schema and metadata
    └── indexes.meta            # Index definitions
```

**Index Persistence Strategy**:
- **Write-Through**: Index updates immediately written to disk
- **Write-Behind**: Batch index updates for performance
- **Memory-Mapped**: Large indexes can be memory-mapped
- **Crash Recovery**: Rebuild indexes from record data if corrupted

**Index Memory Management**:
```rust
struct TableIndexes {
    primary: PrimaryIndex,                          // Always in memory
    secondary: HashMap<String, Box<dyn SecondaryIndex>>, // Configurable
    
    // Memory management
    memory_limit: usize,                           // Max memory for indexes
    cache_policy: IndexCachePolicy,               // LRU, LFU, etc.
}
```

### Query Processing

**Query Execution Flow**:
```
Query Types and Execution:

1. Point Query (by Record ID):
   Record ID → Primary Index → Slot Position → Record Data
   
2. Secondary Key Query:
   Field Value → Secondary Index → Record IDs → Primary Index → Slot Positions → Record Data
   
3. Range Query:
   Range → B+ Tree Index → Record IDs → Primary Index → Slot Positions → Record Data
   
4. Composite Query:
   Multi-Field Values → Composite Index → Record IDs → Primary Index → Record Data
```

**Query Optimization**:
- **Index Selection**: Choose most selective index for query
- **Index Intersection**: Combine multiple indexes for complex queries
- **Parallel Lookup**: Parallelize primary index lookups
- **Caching**: Cache frequently accessed record IDs and slot positions

**Example Query Processing**:
```rust
impl Table {
    // Query by secondary index
    fn find_by_user_id(&self, user_id: u32) -> Result<Vec<&Record>> {
        // 1. Use secondary index to find record IDs
        let record_ids = self.secondary_indexes
            .get("user_id")?
            .find(&FieldValue::U32(user_id));
        
        // 2. Use primary index to find slot positions
        let mut records = Vec::new();
        for record_id in record_ids {
            if let Some(slot) = self.primary_index.get(record_id) {
                if let Some(record) = self.get_slot(slot) {
                    if record.is_active() {
                        records.push(record);
                    }
                }
            }
        }
        
        Ok(records)
    }
    
    // Range query
    fn find_by_timestamp_range(&self, start: u64, end: u64) -> Result<Vec<&Record>> {
        let record_ids = self.secondary_indexes
            .get("timestamp")?
            .range(&FieldValue::U64(start), &FieldValue::U64(end));
            
        // Convert to records using primary index
        self.lookup_records_by_ids(record_ids)
    }
}
```

**Performance Characteristics**:
```
Operation Type        | Primary Index | Secondary Index | Total Complexity
----------------------|---------------|-----------------|------------------
Point Query (by ID)   | O(1)         | N/A            | O(1)
Secondary Key Query   | O(1)         | O(1)           | O(1) + O(k) where k=results
Range Query           | O(1)         | O(log n + k)   | O(log n + k)
Composite Query       | O(1)         | O(log n + k)   | O(log n + k)
Insert                | O(1)         | O(log n)       | O(log n)
Update                | O(1)         | O(log n)       | O(log n)
Delete                | O(1)         | O(log n)       | O(log n)
```

## Advanced Features

### Multi-Version Concurrency (MVCC)

The Table Manager coordinates with the Transaction Manager to provide MVCC for concurrent access.

**Version Chain Management**:
```
Record Versions within Table:
Latest ──→ Version 2 ──→ Version 1 ──→ Version 0
  │           │            │            │
Active      TX-127       TX-123       TX-100
Reader      (visible)    (historical) (old)
```

**How MVCC Works with Tables**:
- **Per-Record Versioning**: Each record can have multiple versions
- **Transaction Coordination**: Table Manager works with Transaction Manager
- **Version Visibility**: Transactions see appropriate record versions
- **Garbage Collection**: Old versions cleaned up automatically

**Table-Level MVCC Operations**:
```rust
impl Table {
    fn read_at_timestamp(&self, record_id: RecordId, timestamp: u64) -> Option<&Record> {
        let versions = self.get_version_chain(record_id)?;
        versions.find_version_at_timestamp(timestamp)
    }
    
    fn create_new_version(&mut self, record_id: RecordId, data: &[u8]) -> Result<()> {
        // Create new version while keeping old ones for concurrent readers
        let new_version = self.allocate_version_slot()?;
        self.write_version(new_version, data)?;
        self.link_to_version_chain(record_id, new_version)?;
        Ok(())
    }
}
```

**Benefits**:
- **No Read Blocks**: Readers never block writers or other readers
- **Consistent Snapshots**: Each transaction sees consistent data
- **High Concurrency**: Multiple transactions can operate simultaneously
- **Table Isolation**: MVCC handled independently per table

### Schema Management

Each table has its own schema that defines the structure of its records.

**Table Schema Components**:
- **Field Definitions**: Name, type, offset, size for each field
- **Schema Version**: Track evolution over time
- **Validation Rules**: Data type and business rule validation
- **Default Values**: Used when adding new fields

**Schema Evolution Process**:
```rust
impl Table {
    fn evolve_schema(&mut self, changes: SchemaChanges) -> Result<()> {
        // Validate changes are backward compatible
        self.validate_schema_changes(&changes)?;
        
        // Update schema definition
        let new_version = self.schema.version + 1;
        let new_schema = self.schema.apply_changes(changes, new_version)?;
        
        // All new records use new schema
        self.schema = new_schema;
        
        // Old records still work (lazy migration)
        Ok(())
    }
    
    fn validate_record(&self, data: &[u8]) -> Result<()> {
        self.schema.validate_fields(data)?;
        self.schema.check_business_rules(data)?;
        Ok(())
    }
}
```

**Schema Change Types**:
- **Add Field**: New fields get default values for existing records
- **Deprecate Field**: Mark as unused but don't delete (backward compatibility)
- **Modify Constraints**: Update validation rules
- **Version Bump**: Increment schema version

**Migration Strategy**:
- **Lazy Migration**: Records migrated when accessed
- **Batch Migration**: Migrate all records during maintenance
- **Version Coexistence**: Multiple schema versions can coexist
- **Rollback Support**: Can revert to previous schema version

### Zero-Copy Storage

Each table implements zero-copy storage for maximum performance.

**Table-Level Zero-Copy Implementation**:
- **Direct Memory Layout**: Records stored in CPU-native binary format
- **No Serialization**: Data ready for immediate use by applications
- **Memory Alignment**: Fields aligned for optimal CPU cache performance
- **Type-Safe Access**: Schema ensures correct data interpretation

**How Tables Store Data**:
```rust
impl Table {
    fn write_record_direct(&mut self, slot: SlotIndex, data: &RecordData) -> Result<()> {
        let memory_addr = self.get_slot_address(slot);
        
        // Write header directly to memory
        self.write_header(memory_addr, &data.header)?;
        
        // Write payload directly (no serialization)
        self.write_payload(memory_addr + HEADER_SIZE, &data.payload)?;
        
        // Write footer with checksum
        self.write_footer(memory_addr + HEADER_SIZE + data.payload.len(), &data.footer)?;
        
        Ok(())
    }
    
    fn read_record_direct(&self, slot: SlotIndex) -> Option<&Record> {
        let memory_addr = self.get_slot_address(slot);
        // Direct pointer to memory - no copying or parsing needed
        unsafe { &*(memory_addr as *const Record) }
    }
}
```

**Field Storage within Tables**:
- **Integers & Floats**: Stored as native binary (u32, f64, etc.)
- **Strings**: Offset + length pointer to string data within record
- **Arrays**: Count + elements (if fixed-size) or offset pointer
- **Enums**: Smallest integer type that fits all variants

**Memory Layout Optimization**:
- **Field Grouping**: 8-byte, 4-byte, 2-byte, 1-byte fields grouped together
- **Cache Line Alignment**: Records aligned to 64-byte boundaries
- **Padding Elimination**: Packed structures minimize wasted space

## Performance

### Data Types and Storage

Tables optimize storage layout for different data types:

**Primitive Types (Direct Storage)**:
- **Integers**: u8, u16, u32, u64, i8, i16, i32, i64 stored as native binary
- **Floating Point**: f32, f64 stored as IEEE 754 binary  
- **Booleans**: Single byte (0 or 1)
- **Timestamps**: i64 milliseconds since epoch
- **Enums**: Smallest integer type that accommodates all variants

**Complex Types (Offset-Based)**:
- **Strings**: [offset: u16, length: u16] + UTF-8 bytes in payload area
- **Binary Blobs**: [offset: u16, length: u16] + raw bytes in payload area
- **Arrays**: Element count + elements (if fixed-size) or offset to data
- **Optional Fields**: Null bitmask + conditional storage

**Table-Specific Optimizations**:
```rust
// Example: Orders table layout optimized for trading
struct OrderRecord {
    // Hot fields first (frequently accessed)
    order_id: u64,           // 8 bytes
    price: u64,              // 8 bytes (fixed-point)
    quantity: u64,           // 8 bytes
    timestamp: u64,          // 8 bytes
    
    // 4-byte fields grouped together
    user_id: u32,            // 4 bytes
    symbol_offset: u16,      // 2 bytes
    symbol_len: u16,         // 2 bytes
    
    // Smaller fields at end
    order_type: u8,          // 1 byte
    status: u8,              // 1 byte
    flags: u16,              // 2 bytes
    
    // Total: 48 bytes + variable string data
}
```

### Performance Comparison

**Storage Engine Comparison (Page-Aware + Indexes)**:
```
Storage Method         | Write Time | Read Time | Memory Use        | Page Efficiency | Index Overhead
-----------------------|------------|-----------|-------------------|-----------------|----------------
Table Manager (Optimized)| ~200ns  | ~50ns     | Table + Indexes   | 100%            | ~24B per record
Traditional Database   | ~5-20ms   | ~2-10ms   | Variable + Indexes| Variable        | High
JSON + Parsing         | ~5μs      | ~3μs      | Text overhead     | N/A             | External indexes
Protocol Buffers       | ~2μs      | ~1.5μus   | Compact binary    | N/A             | External indexes
Redis (In-Memory)      | ~1μs      | ~800ns    | Key-value + hash  | N/A             | Built-in hash
```

**Table Performance with Page-Aware Design**:
```
Table Type    | Record Size | Records/Page | Index Size | Read Amplification | Query Throughput
--------------|-------------|--------------|------------|--------------------|------------------
Users         | 256B        | 16 records   | ~24B       | 1x (batch) / 16x   | 1M+ point queries
Orders        | 512B        | 8 records    | ~24B       | 1x (batch) / 8x    | 500K+ range queries
Logs          | 128B        | 32 records   | ~24B       | 1x (batch) / 32x   | 2M+ indexed inserts
Config        | 1024B       | 4 records    | ~24B       | 1x (batch) / 4x    | 100K+ updates
```

**Index Performance Breakdown (Page-Aware)**:
```
Operation Type          | Primary Index | Secondary Index | Page Load | Total Time (Single/Batch)
------------------------|---------------|-----------------|-----------|-------------------------
Point Query (by ID)     | ~30ns        | N/A            | ~200ns    | ~250ns / ~50ns (amortized)
Secondary Field Query   | ~30ns        | ~100ns         | ~200ns    | ~330ns / ~130ns (amortized)
Range Query            | ~30ns        | ~200ns         | ~500ns    | ~730ns / ~200ns (batch)
Insert (with indexes)   | ~50ns        | ~150ns         | ~300ns    | ~500ns / ~200ns (batch)
Update (with indexes)   | ~50ns        | ~300ns         | ~300ns    | ~650ns / ~300ns (batch)
Delete (with indexes)   | ~50ns        | ~150ns         | ~200ns    | ~400ns / ~150ns (batch)
```

**Architectural Benefits (Page-Aware Design)**:
```
Aspect              | Traditional DB      | Table Manager (Page-Aware)
--------------------|--------------------|--------------------------
Data Access         | SQL parse + execute | Direct memory read
Memory Layout       | Row-based mixed     | Page-aligned, type-grouped
Fragmentation       | Variable row sizes  | Fixed slots per table
Type Safety         | Runtime validation  | Compile-time schema
Transaction Scope   | Database-wide       | Table-level isolation
Scaling             | Vertical mainly     | Horizontal per table
Page Efficiency     | Variable            | 100% (perfect divisors)
Read Amplification  | High                | 1-32x (batch optimized)
Write Amplification | High                | 1-32x (batch optimized)
Cache Utilization   | Poor                | Excellent (page locality)
```

### Optimization Strategies

**Table-Level Optimizations**:
- **Page-Aligned Records**: Choose record sizes as perfect divisors of 4KB
- **Hot Field Grouping**: Place frequently accessed fields at record start
- **Cache Line Alignment**: Align records to 64-byte CPU cache boundaries  
- **NUMA Locality**: Allocate tables on local NUMA nodes
- **Batch Operations**: Process multiple records within same page
- **Page Locality**: Allocate related records in same pages when possible

**Memory Management**:
- **Page-Aware Pools**: Each table manages page-aligned memory regions  
- **Page-Level Allocation**: Allocate records in page-aware manner
- **Prefault Pages**: Pre-fault memory pages during table initialization
- **Huge Pages**: Use 2MB/1GB pages to reduce TLB pressure
- **Memory Locking**: Lock critical tables in physical memory
- **Hot Page Caching**: Keep frequently accessed pages in faster memory

**Storage Tiering**:
- **Hot Tables**: Active tables on local NVMe storage
- **Warm Tables**: Infrequently accessed tables on network storage
- **Cold Tables**: Archived tables with compression on object storage

## Page-Aware Storage Design

### Read/Write Amplification Problem

**The Core Issue**:
Memory-mapped files operate at the OS page level (typically 4KB), but our records might be much smaller. This creates a significant read/write amplification problem.

**Amplification Analysis**:
```
OS Page Size: 4KB (4096 bytes)
Record Sizes and Amplification:

Record Size | Records/Page | Read Amplification | Write Amplification
------------|--------------|--------------------|--------------------- 
128B        | 32 records   | 32x (4096/128)     | Up to 32x
256B        | 16 records   | 16x (4096/256)     | Up to 16x  
512B        | 8 records    | 8x (4096/512)      | Up to 8x
1024B       | 4 records    | 4x (4096/1024)     | Up to 4x
2048B       | 2 records    | 2x (4096/2048)     | Up to 2x
4096B       | 1 record     | 1x (perfect)       | 1x (perfect)
```

**Real-World Impact**:
- **Single Record Access**: Reading 128B record loads entire 4KB page (32x amplification)
- **Single Record Update**: Modifying 128B record may cause 4KB page writeback
- **Cache Pollution**: Unwanted data loaded into CPU caches
- **Memory Bandwidth**: Wasted memory bandwidth on unused data
- **I/O Overhead**: Higher I/O load than necessary

**Why This Matters for Performance**:
```
Example: Orders table with 128B records
- Application reads 1 order
- OS loads 4KB page (32 orders)  
- CPU cache filled with 31 unwanted orders
- Memory bandwidth: 32x higher than needed
- If page is dirty: 4KB write for 128B change
```

### Page-Aligned Record Layout

**Strategy 1: Optimal Record Sizing**:
Design record sizes to be friendly divisors of 4KB page size.

**Page-Friendly Record Sizes**:
```rust
// Optimal record sizes for 4KB pages
const PAGE_SIZE: usize = 4096;

// Perfect divisors (no wasted space)
const RECORD_SIZE_512B: usize = 512;   // 8 records per page
const RECORD_SIZE_1024B: usize = 1024; // 4 records per page  
const RECORD_SIZE_2048B: usize = 2048; // 2 records per page
const RECORD_SIZE_4096B: usize = 4096; // 1 record per page

// Good divisors (minimal waste)
const RECORD_SIZE_256B: usize = 256;   // 16 records per page
const RECORD_SIZE_1365B: usize = 1365; // 3 records per page (3B waste)
```

**Table Design with Page Awareness**:
```rust
struct PageAwareTable {
    record_size: usize,
    records_per_page: usize,
    page_count: usize,
    
    // Page-aligned memory mapping
    memory_map: MemoryMap,
    
    // Page-level metadata
    page_headers: Vec<PageHeader>,
    
    // Hot page tracking
    hot_pages: HashSet<PageIndex>,
}

struct PageHeader {
    page_index: u32,
    used_slots: u16,        // How many records in this page
    free_slots: u16,        // Available slots
    dirty: bool,            // Page has been modified
    access_count: u32,      // For hot page detection
    last_access: Timestamp,
}
```

**Strategy 2: Page-Level Record Organization**:
```rust
impl PageAwareTable {
    fn allocate_record(&mut self) -> Result<(PageIndex, SlotIndex)> {
        // Try to allocate in a page with existing records (better cache locality)
        if let Some(page_idx) = self.find_page_with_space() {
            let slot_idx = self.allocate_slot_in_page(page_idx)?;
            return Ok((page_idx, slot_idx));
        }
        
        // Allocate new page if needed
        let page_idx = self.allocate_new_page()?;
        let slot_idx = 0; // First slot in new page
        Ok((page_idx, slot_idx))
    }
    
    fn find_page_with_space(&self) -> Option<PageIndex> {
        // Prefer hot pages with available space
        for &page_idx in &self.hot_pages {
            if self.page_headers[page_idx].free_slots > 0 {
                return Some(page_idx);
            }
        }
        
        // Fall back to any page with space
        self.page_headers
            .iter()
            .position(|header| header.free_slots > 0)
    }
}
```

### Batch Operations

**Page-Level Batch Processing**:
Instead of processing records individually, process entire pages or multiple records within a page.

**Batch Read Operations**:
```rust
impl Table {
    // Read all records in a page
    fn read_page(&self, page_idx: PageIndex) -> Result<Vec<&Record>> {
        let page_header = &self.page_headers[page_idx];
        let mut records = Vec::with_capacity(page_header.used_slots as usize);
        
        let page_base = self.get_page_address(page_idx);
        for slot_idx in 0..self.records_per_page {
            if self.is_slot_used(page_idx, slot_idx) {
                let record_addr = page_base + (slot_idx * self.record_size);
                let record = unsafe { &*(record_addr as *const Record) };
                records.push(record);
            }
        }
        
        Ok(records)
    }
    
    // Batch query within page range
    fn query_page_range(&self, start_page: PageIndex, end_page: PageIndex, filter: &dyn Fn(&Record) -> bool) -> Vec<&Record> {
        let mut results = Vec::new();
        
        for page_idx in start_page..=end_page {
            // Load entire page at once (amortize page load cost)
            if let Ok(page_records) = self.read_page(page_idx) {
                for record in page_records {
                    if filter(record) {
                        results.push(record);
                    }
                }
            }
        }
        
        results
    }
}
```

**Batch Write Operations**:
```rust
impl Table {
    // Insert multiple records in same page
    fn batch_insert_in_page(&mut self, page_idx: PageIndex, records: Vec<&[u8]>) -> Result<Vec<RecordId>> {
        let available_slots = self.page_headers[page_idx].free_slots as usize;
        if records.len() > available_slots {
            return Err(Error::PageFull);
        }
        
        let mut record_ids = Vec::with_capacity(records.len());
        
        // Mark page as dirty once
        self.page_headers[page_idx].dirty = true;
        
        for data in records {
            let slot_idx = self.find_free_slot_in_page(page_idx)?;
            let record_id = self.generate_snowflake_id();
            
            self.write_record_to_slot(page_idx, slot_idx, record_id, data)?;
            self.update_indexes(record_id, page_idx, slot_idx)?;
            
            record_ids.push(record_id);
        }
        
        // Update page header once
        self.page_headers[page_idx].used_slots += records.len() as u16;
        self.page_headers[page_idx].free_slots -= records.len() as u16;
        
        Ok(record_ids)
    }
}
```

### Hot Page Management

**Hot Page Detection and Optimization**:
```rust
struct HotPageManager {
    // Track page access patterns
    page_access_stats: HashMap<PageIndex, PageAccessStats>,
    
    // Currently hot pages (kept in faster storage/memory)
    hot_pages: LruCache<PageIndex, PageData>,
    
    // Configuration
    hot_threshold: u32,        // Access count to be considered hot
    hot_page_limit: usize,     // Max number of hot pages to track
}

struct PageAccessStats {
    access_count: u32,
    last_access: Timestamp,
    access_pattern: AccessPattern, // Sequential, Random, etc.
}

impl HotPageManager {
    fn on_page_access(&mut self, page_idx: PageIndex) {
        let stats = self.page_access_stats.entry(page_idx).or_default();
        stats.access_count += 1;
        stats.last_access = now();
        
        // Promote to hot if threshold reached
        if stats.access_count >= self.hot_threshold {
            self.promote_to_hot_page(page_idx);
        }
    }
}
```

**Page-Aware Query Optimization**:
```rust
impl Table {
    fn optimize_query_for_pages(&self, record_ids: Vec<RecordId>) -> Vec<&Record> {
        // Group record IDs by their page location
        let mut records_by_page: HashMap<PageIndex, Vec<(SlotIndex, RecordId)>> = HashMap::new();
        
        for record_id in record_ids {
            if let Some(slot_idx) = self.primary_index.get(record_id) {
                let page_idx = slot_idx / self.records_per_page;
                let page_slot = slot_idx % self.records_per_page;
                records_by_page.entry(page_idx).or_default().push((page_slot, record_id));
            }
        }
        
        let mut results = Vec::new();
        
        // Process each page once, collecting all needed records
        for (page_idx, page_records) in records_by_page {
            // Load page once
            let page_base = self.get_page_address(page_idx);
            
            // Collect all records from this page
            for (slot_idx, _record_id) in page_records {
                let record_addr = page_base + (slot_idx * self.record_size);
                let record = unsafe { &*(record_addr as *const Record) };
                results.push(record);
            }
        }
        
        results
    }
}
```

**Performance Impact of Page-Aware Design**:
```
Optimization Strategy          | Read Amplification | Write Amplification | Performance Gain
------------------------------|--------------------|--------------------|------------------
Original (128B records)       | 32x               | 32x                | Baseline
Page-Aligned (512B records)   | 8x                | 8x                 | 4x improvement
Batch Operations (8 at once)  | 4x                | 4x                 | 8x improvement
Hot Page Caching              | 1x (for hot data) | 1x (for hot data) | 10-32x improvement
Combined Optimizations         | 1-4x              | 1-4x               | 10-30x improvement
```

## Data Integrity

**Table-Level Validation**:
- **Schema Validation**: Each table validates records against its schema
- **Business Rules**: Table-specific validation logic
- **Reference Integrity**: Cross-table reference validation
- **Data Quality**: Range, format, and constraint checking

**Integrity Protection Mechanisms**:
- **Record Checksums**: CRC32 checksums for fast corruption detection
- **Table Checksums**: Validate entire table integrity periodically
- **Cryptographic Hashes**: Strong protection for sensitive tables
- **Error Correction**: Reed-Solomon codes for critical data
- **Audit Trails**: Track all changes for compliance and debugging

**Table-Level Integrity Operations**:
```rust
impl Table {
    fn validate_integrity(&self) -> Result<()> {
        // Check all record checksums
        for slot in self.used_slots() {
            let record = self.get_slot(slot);
            record.verify_checksum()?;
        }
        
        // Validate table-level constraints
        self.schema.validate_table_constraints(self)?;
        
        // Check cross-references if any
        self.validate_foreign_keys()?;
        
        Ok(())
    }
}
```

## Catalog System

The catalog manages metadata about all tables and schemas in the Table Manager.

### Catalog Structure

**Table Registry**:
- **Table Metadata**: Name, ID, record size, schema version
- **Storage Location**: File paths and memory pool information  
- **Schema Definitions**: Current and historical schema versions
- **Index Registry**: Secondary indexes for each table
- **Statistics**: Record counts, access patterns, performance metrics

**Catalog Data Structure**:
```rust
struct TableCatalog {
    // Table registry for fast lookup
    tables: HashMap<String, TableInfo>,
    
    // Schema cache for validation
    schemas: HashMap<u32, Schema>,
    
    // Index registry per table
    table_indexes: HashMap<TableId, IndexRegistry>,
    
    // System metadata
    system_info: SystemMetadata,
}

struct TableInfo {
    table_id: u32,
    name: String,
    record_size: u32,
    schema_version: u32,
    file_path: PathBuf,
    memory_pool: MemoryPoolInfo,
    index_files: IndexFileInfo,        // New: index file locations
    statistics: TableStats,
    created_at: Timestamp,
}

struct IndexRegistry {
    primary_index: PrimaryIndexInfo,
    secondary_indexes: HashMap<String, SecondaryIndexInfo>,
    index_memory_usage: usize,
}

struct IndexFileInfo {
    primary_index_file: PathBuf,       // primary.idx
    secondary_index_dir: PathBuf,      // indexes/
    index_metadata_file: PathBuf,      // indexes.meta
}
```

### Catalog Operations

**Core Table Operations**:
```rust
impl TableManager {
    // Create new table with schema and indexes
    fn create_table(&mut self, name: String, schema: Schema) -> Result<TableId> {
        let table_info = TableInfo::new(name, schema)?;
        
        // Register table in catalog
        self.catalog.register_table(table_info)?;
        
        // Initialize table storage
        self.initialize_table_storage(&table_info)?;
        
        // Create primary index
        self.create_primary_index(table_info.table_id)?;
        
        // Create default secondary indexes based on schema
        self.create_default_secondary_indexes(table_info.table_id, &table_info.schema)?;
        
        Ok(table_info.table_id)
    }
    
    // Get table by name for operations  
    fn get_table(&self, name: &str) -> Option<&Table> {
        let table_info = self.catalog.get_table_info(name)?;
        self.tables.get(&table_info.table_id)
    }
    
    // Create secondary index on table
    fn create_index(&mut self, table_name: &str, field_name: &str, index_type: IndexType) -> Result<()> {
        let table = self.get_table_mut(table_name)?;
        let index_info = SecondaryIndexInfo {
            name: format!("{}_{}", table_name, field_name),
            field: field_name.to_string(),
            index_type,
        };
        
        // Create and build the index
        let index = self.build_secondary_index(&index_info, table)?;
        table.add_secondary_index(index_info.name.clone(), index);
        
        // Update catalog
        self.catalog.register_secondary_index(table.table_id, index_info)?;
        
        Ok(())
    }
    
    // Update table schema (handles index updates)
    fn evolve_table_schema(&mut self, name: &str, changes: SchemaChanges) -> Result<()> {
        let table = self.get_table_mut(name)?;
        
        // Check if schema changes affect existing indexes
        let affected_indexes = self.find_affected_indexes(&table.table_id, &changes)?;
        
        // Update schema
        table.evolve_schema(changes)?;
        self.catalog.update_schema(name, &table.schema)?;
        
        // Rebuild affected secondary indexes
        for index_name in affected_indexes {
            self.rebuild_secondary_index(&table.table_id, &index_name)?;
        }
        
        Ok(())
    }
}
```

### Catalog Persistence

**Persistence Strategy**:
- **Atomic Updates**: All catalog changes written atomically
- **Recovery Support**: Catalog rebuilds table registry on startup  
- **Version Control**: Track schema evolution over time
- **Backup Integration**: Catalog metadata included in snapshots

**Bootstrap Sequence**:
1. Load catalog metadata from persistent storage
2. Validate all table files and index files exist and are accessible  
3. Initialize table memory pools and allocation bitmaps
4. Load primary indexes into memory (critical for O(1) access)
5. Load or rebuild secondary indexes based on configuration
6. Verify index consistency with record data
7. Mark catalog and indexes as ready for operations

**Index Recovery Strategy**:
- **Primary Index**: Always rebuilt if corrupted (scan all records)
- **Secondary Indexes**: Can be rebuilt from primary index + record data  
- **Crash Recovery**: Detect incomplete index updates and replay/rollback
- **Consistency Check**: Verify index entries match actual record slots
