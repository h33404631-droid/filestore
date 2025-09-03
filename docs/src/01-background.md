# Background

## Problem Statement

In cryptocurrency trading systems, the OrderManager component faces significant performance challenges during high-volume market conditions. When market trends trigger rapid price movements, thousands of users can simultaneously submit orders, creating intense database load. Even with traditional solutions like database sharding and horizontal scaling, the system may still experience latency spikes that cause users to miss time-sensitive trading opportunities.

This document analyzes the storage requirements for a high-performance order management system and proposes optimizations to achieve sub-millisecond latency.

## Requirements Analysis

### Performance Requirements
- **Latency**: Target p99 latency < 10ms for order operations
- **Throughput**: Support 100,000+ orders per second during peak trading
- **Availability**: 99.99% uptime with minimal impact from storage operations

### Data Characteristics
The system primarily handles "live order" data with specific access patterns:

1. **Write-Heavy Workload**: Orders are frequently inserted as users place new trades
2. **Read-Heavy Queries**: Users continuously query their active orders and market depth
3. **Rare Updates**: Orders are seldom modified (price adjustments, partial cancellations)
4. **Lifecycle Management**: Completed orders are archived to cold storage (e.g., ClickHouse) for analytics

### Consistency Requirements
- **Strong Consistency**: Critical for order placement and matching
- **Eventual Consistency**: Acceptable for non-critical read operations
- **Durability**: Orders must survive system crashes without data loss

## Current Architecture Limitations

### Traditional RDBMS Approach (MySQL)
A typical MySQL-based order management system involves multiple I/O operations per transaction:

1. **Network Round-trip**: Application ↔ MySQL server communication
2. **Transaction Log (binlog)**: Write-ahead logging for durability  
3. **Data Pages**: Actual row data writes to storage
4. **Index Updates**: B-tree index maintenance for quick lookups
5. **Replication**: If using master-slave setup for high availability

**Total I/O Analysis**:
- 1 network operation (typically 0.1-1ms in local datacenter)
- 2-4 disk operations (1-10ms each on traditional storage)
- **Result**: 2-40ms total latency per operation

### Embedded Database Approach (RocksDB)
While embedded databases eliminate network overhead, they still face challenges:

**Advantages**:
- No network latency between application and storage
- Optimized LSM-tree structure for write-heavy workloads
- Efficient compression and bloom filters

**Limitations**:
- Write-Ahead Log (WAL) still requires disk I/O for durability
- Cloud block storage adds network latency (EBS, persistent disks)
- LSM compaction can cause periodic latency spikes

## Storage I/O Analysis

### Disk Storage Performance
- **Cloud Block Storage (EBS, GCP PD)**: 1-10ms latency
- **Local NVMe SSDs**: 0.1-1ms latency  
- **Memory**: 50-100 nanoseconds latency

### The I/O Bottleneck
Storage I/O remains the primary performance bottleneck because:
1. **Mechanical Limitations**: Even SSDs have microsecond-level latencies
2. **Durability vs. Performance Trade-off**: WAL writes ensure crash recovery but add latency
3. **Cloud Storage Overhead**: Network-attached storage adds round-trip time

## 系统架构分析

### 传统架构的根本性局限

传统数据库架构在交易系统场景下存在以下根本性问题：

#### 多层抽象开销
```
传统数据库请求路径:
应用程序 → SQL解析 → 查询优化 → 执行计划 → 存储引擎 → 缓冲池 → 磁盘I/O
每一层都增加延迟: ~500μs + ~200μs + ~300μs + ~100μs + ~50μs + ~2ms = ~3.15ms
```

#### 通用性 vs 专用性权衡
- **通用数据库**: 设计为支持各种工作负载，但在特定场景下不够优化
- **交易系统需求**: 高度专门化的数据访问模式和性能要求
- **解决方案**: 为特定工作负载定制存储引擎

### 性能瓶颈深度分析

#### 延迟分解分析
针对p99 < 10ms的目标，我们需要分解每个组件的延迟贡献：

```
目标延迟预算分配:
┌─────────────────────────────────────────┐
│ 总目标延迟: 10ms (内网优化)              │
├─────────────────────────────────────────┤
│ 网络传输:     0.1ms (1%)                │
│ 接口层:       0.5ms (5%)                │
│ 存储引擎:     6ms (60%)                 │
│ 持久化层:     0.5ms (5%)                │
│ 硬件层:       0.4ms (4%)                │
│ 缓冲时间:     2.5ms (25%)               │
└─────────────────────────────────────────┘
```

#### I/O延迟层次分析
```
存储介质延迟对比:
CPU L1缓存:     ~1ns     (基准)
CPU L2缓存:     ~3ns     (3x)
CPU L3缓存:     ~12ns    (12x)
系统内存:       ~100ns   (100x)
本地NVMe SSD:   ~25μs    (25,000x)
网络SSD:       ~100μs   (100,000x)
SATA SSD:      ~500μs   (500,000x)
机械硬盘:      ~10ms    (10,000,000x)
```

### 架构设计原则

#### 1. 分层优化策略
```
性能优化路径图:
阶段1: 消除SQL层 (50ms → 10ms)
  ├── 直接二进制协议
  ├── 预编译查询
  └── 连接池优化

阶段2: 内存化存储 (10ms → 5ms)
  ├── 内存映射文件
  ├── 零拷贝I/O
  └── 固定大小记录

阶段3: 硬件优化 (5ms → 2ms)
  ├── CPU缓存对齐
  ├── NUMA感知分配
  └── 批处理操作
```

#### 2. 专用化设计理念
- **单一职责**: 每个表专门存储一种业务对象
- **固定结构**: 消除动态内存分配和碎片化
- **类型安全**: 编译时确定的数据结构
- **直接访问**: 绕过传统数据库的抽象层

## 解决方案架构概述

### 核心设计哲学

#### Memory-First架构
- **热数据常驻内存**: 活跃交易数据完全存在RAM中
- **分层存储**: 热/温/冷数据自动分层管理
- **异步持久化**: 将持久化从关键路径中解耦

#### 硬件感知设计
- **NUMA拓扑感知**: 数据和计算绑定到相同NUMA节点
- **CPU缓存友好**: 数据结构对齐到缓存行边界
- **存储介质优化**: 针对NVMe SSD特性优化I/O模式

#### 专用存储引擎
- **表驱动架构**: 每种业务对象一个专用表
- **零抽象开销**: 直接内存访问，无序列化
- **编译时优化**: 利用Rust零成本抽象特性

### 系统分层架构

```
FileStore存储引擎分层设计:

┌─────────────────────────────────────────┐
│ 🌐 接口层 (API Gateway Layer)           │
│   - gRPC/HTTP多协议支持                 │
│   - 负载均衡和服务发现                   │
│   - 认证授权和限流                       │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ 💼 业务服务层 (Business Service Layer)  │
│   - 订单生命周期管理                     │
│   - 业务规则验证                         │
│   - 事件发布和状态管理                   │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ 🔧 数据服务层 (Data Service Layer)      │
│   - 表管理和查询执行                     │
│   - 事务协调和MVCC                      │
│   - 索引管理和优化                       │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ 🏗️ 存储引擎层 (Storage Engine Layer)    │
│   - 内存池管理和页面分配                 │
│   - 并发控制和锁管理                     │
│   - 版本链和垃圾回收                     │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ 💾 持久化层 (Persistence Layer)        │
│   - WAL和快照管理                       │
│   - 异步刷盘和恢复                       │
│   - 备份和归档                           │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ ⚡ 硬件抽象层 (Hardware Layer)          │
│   - NVMe直接访问                        │
│   - NUMA内存管理                        │
│   - CPU缓存优化                         │
└─────────────────────────────────────────┘
```

### 关键技术决策

#### 1. 内存映射 vs 传统I/O
**选择**: 内存映射文件 (mmap)
**理由**: 
- 消除用户态/内核态切换开销
- 利用OS页面缓存机制
- 支持零拷贝数据访问
- 自动内存管理和换页

#### 2. 固定 vs 变长记录
**选择**: 固定大小记录
**理由**:
- 消除内存碎片化
- O(1)地址计算
- 高效的内存预取
- 简化并发控制

#### 3. 同步 vs 异步持久化
**选择**: 混合策略
**关键路径**: 同步到WAL (本地NVMe, ~500μs)
**数据文件**: 异步刷盘 (后台批量写入)
**快照**: 定期异步快照 (不阻塞业务)

## 下一步设计细节

The following sections will detail our approach to building a low-latency, persistent storage system that meets these requirements through:

1. **分层架构设计**: 详细的组件职责划分和接口定义
2. **存储引擎核心**: 内存管理、索引系统、并发控制的具体实现
3. **性能优化策略**: 从硬件到应用层的全栈优化方案
4. **分布式系统考虑**: 可扩展性、一致性、容错性的平衡设计
