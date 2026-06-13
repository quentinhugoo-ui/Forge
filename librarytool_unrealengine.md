# Unreal Engine - Arborescence Tools

```text
Scope
|-- Official Unreal tools/features = docs Epic
|-- Built-in plugins = Plugin Index Epic
`-- External/indie plugins = exemples verifies par sources, non exhaustif
```

```text
Unreal Engine User Tools
|-- 00 Core Editor
|   |-- Epic Games Launcher
|   |-- Project Browser
|   |-- Unreal Editor
|   |   |-- Main Menu
|   |   |-- Toolbar
|   |   |-- Viewports
|   |   |-- Docking / Tabs
|   |   |-- Details Panel
|   |   |-- World Outliner
|   |   |-- Content Drawer
|   |   |-- Content Browser
|   |   |-- Output Log
|   |   |-- Message Log
|   |   |-- Console
|   |   |-- Editor Preferences
|   |   `-- Project Settings
|   |-- Level Editor
|   |   |-- Select Mode
|   |   |-- Viewport Toolbar
|   |   |-- Transform Gizmo
|   |   |-- Translate / Rotate / Scale
|   |   |-- Grid Snap
|   |   |-- Rotation Snap
|   |   |-- Scale Snap
|   |   |-- Surface Snap
|   |   |-- Pivot Editing
|   |   |-- Actor Placement
|   |   |-- Actor Grouping
|   |   |-- Actor Folders
|   |   |-- Layers
|   |   |-- Bookmarks
|   |   |-- World Settings
|   |   |-- Play In Editor
|   |   |-- Simulate In Editor
|   |   |-- Standalone Game
|   |   `-- Multiplayer PIE
|   |-- Modes
|   |   |-- Select
|   |   |-- Landscape
|   |   |-- Foliage
|   |   |-- Modeling
|   |   |-- Mesh Paint
|   |   |-- Fracture
|   |   |-- Animation
|   |   |-- Scriptable Tools
|   |   `-- Custom Plugin Modes
|   |-- Content Browser
|   |   |-- Asset Search
|   |   |-- Filters
|   |   |-- Collections
|   |   |-- Favorites
|   |   |-- Asset Actions
|   |   |-- Bulk Edit
|   |   |-- Rename
|   |   |-- Duplicate
|   |   |-- Migrate
|   |   |-- Reimport
|   |   |-- Validate Assets
|   |   |-- Reference Viewer
|   |   |-- Size Map
|   |   |-- Audit Assets
|   |   |-- Fix Up Redirectors
|   |   `-- Asset Registry Views
|   |-- Plugin Browser
|   |-- Fab / Marketplace Browser
|   |-- Source Control
|   |-- Revision Control
|   |-- Multi-User Browser
|   |-- Project Launcher
|   |-- Device Manager
|   `-- Session Frontend
|
|-- 01 World Building
|   |-- Place Actors
|   |   |-- Basic
|   |   |-- Lights
|   |   |-- Cinematic
|   |   |-- Visual Effects
|   |   |-- Geometry
|   |   |-- Volumes
|   |   |-- All Classes
|   |   `-- Recently Placed
|   |-- Levels
|   |   |-- Persistent Level
|   |   |-- Sublevels
|   |   |-- Level Streaming
|   |   |-- Level Instances
|   |   |-- Packed Level Actors
|   |   `-- Data Layers
|   |-- World Partition
|   |   |-- World Partition Editor
|   |   |-- Runtime Grid
|   |   |-- Streaming Sources
|   |   |-- Data Layers
|   |   |-- HLOD
|   |   |-- One File Per Actor
|   |   |-- Minimap Builder
|   |   `-- World Partition Builder Commandlets
|   |-- Landscape
|   |   |-- New Landscape
|   |   |-- Import Heightmap
|   |   |-- Manage
|   |   |-- Sculpt
|   |   |-- Smooth
|   |   |-- Flatten
|   |   |-- Ramp
|   |   |-- Erosion
|   |   |-- Hydro Erosion
|   |   |-- Noise
|   |   |-- Retopologize
|   |   |-- Visibility
|   |   |-- Paint
|   |   |-- Layer Info
|   |   |-- Splines
|   |   |-- Landmass Brushes
|   |   |-- Landscape Patches
|   |   `-- Landscape Materials
|   |-- Foliage
|   |   |-- Paint Foliage
|   |   |-- Reapply
|   |   |-- Select
|   |   |-- Lasso
|   |   |-- Fill
|   |   |-- Erase
|   |   |-- Static Mesh Foliage
|   |   |-- Actor Foliage
|   |   |-- Procedural Foliage Spawner
|   |   |-- Foliage Type
|   |   `-- Micro Foliage
|   |-- PCG
|   |   |-- PCG Graph Editor
|   |   |-- PCG Component
|   |   |-- PCG Volume
|   |   |-- Input Nodes
|   |   |-- Sampler Nodes
|   |   |-- Filter Nodes
|   |   |-- Spatial Nodes
|   |   |-- Attribute Nodes
|   |   |-- Spawn Nodes
|   |   |-- Subgraphs
|   |   |-- Debug Graph
|   |   |-- Runtime Generation
|   |   |-- PCG Biome
|   |   |-- PCG Water Interop
|   |   |-- PCG Niagara Interop
|   |   `-- PCG Micro Assemblies Interop
|   |-- Water
|   |   |-- Water Body Ocean
|   |   |-- Water Body Lake
|   |   |-- Water Body River
|   |   |-- Water Body Custom
|   |   |-- Water Zone
|   |   |-- Water Mesh
|   |   |-- Buoyancy
|   |   `-- Underwater Post Process
|   |-- Environment
|   |   |-- Sky Atmosphere
|   |   |-- Volumetric Clouds
|   |   |-- Exponential Height Fog
|   |   |-- Local Fog Volumes
|   |   |-- Sky Light
|   |   |-- Directional Light
|   |   |-- Sun Position Calculator
|   |   |-- HDRI Backdrop
|   |   `-- Reflection Captures
|   `-- Geospatial
|       |-- GeoReferencing
|       |-- Cesium For Unreal
|       |-- ArcGIS Maps SDK
|       |-- 3D Tiles
|       `-- Geospatial Coordinates
|
|-- 02 Geometry / Modeling / Mesh
|   |-- Static Mesh Editor
|   |   |-- Mesh Preview
|   |   |-- Materials
|   |   |-- Sockets
|   |   |-- Collision
|   |   |-- Simple Collision
|   |   |-- Complex Collision
|   |   |-- LODs
|   |   |-- Micro Settings
|   |   |-- UV Channels
|   |   |-- Lightmap UVs
|   |   |-- Build Settings
|   |   |-- Distance Field Settings
|   |   `-- Asset Statistics
|   |-- Modeling Mode
|   |   |-- Create
|   |   |   |-- Box
|   |   |   |-- Sphere
|   |   |   |-- Cylinder
|   |   |   |-- Cone
|   |   |   |-- Torus
|   |   |   |-- Disc
|   |   |   |-- Plane
|   |   |   |-- Extrude Polygon
|   |   |   |-- Path Extrude
|   |   |   `-- Revolve
|   |   |-- Select
|   |   |   |-- Triangle Select
|   |   |   |-- PolyGroup Select
|   |   |   |-- Edge Loop
|   |   |   |-- Face Group
|   |   |   `-- Connected Components
|   |   |-- PolyModel
|   |   |   |-- Poly Edit
|   |   |   |-- Extrude
|   |   |   |-- Inset
|   |   |   |-- Outset
|   |   |   |-- Bevel
|   |   |   |-- Bridge
|   |   |   |-- Cut Faces
|   |   |   |-- Insert Edge Loop
|   |   |   `-- Weld Edges
|   |   |-- TriModel
|   |   |   |-- Remesh
|   |   |   |-- Simplify
|   |   |   |-- Smooth
|   |   |   |-- Displace
|   |   |   |-- Offset
|   |   |   |-- Plane Cut
|   |   |   |-- Mirror
|   |   |   `-- Weld
|   |   |-- Deform
|   |   |   |-- Lattice
|   |   |   |-- Bend
|   |   |   |-- Twist
|   |   |   |-- Flare
|   |   |   |-- Warp
|   |   |   |-- Sculpt Deform
|   |   |   `-- Smooth Deform
|   |   |-- Sculpt
|   |   |   |-- Sculpt Brush
|   |   |   |-- Smooth Brush
|   |   |   |-- Move Brush
|   |   |   |-- Inflate Brush
|   |   |   |-- Pinch Brush
|   |   |   |-- Flatten Brush
|   |   |   `-- PolyGroup Paint
|   |   |-- Mesh
|   |   |   |-- Combine
|   |   |   |-- Duplicate
|   |   |   |-- Separate
|   |   |   |-- Split
|   |   |   |-- Trim
|   |   |   |-- Boolean
|   |   |   |-- Self Union
|   |   |   |-- Repair
|   |   |   `-- Fill Holes
|   |   |-- VoxOps
|   |   |   |-- Voxel Merge
|   |   |   |-- Voxel Boolean
|   |   |   |-- Voxel Solidify
|   |   |   |-- Voxel Blend
|   |   |   `-- Voxel Remesh
|   |   |-- Attributes
|   |   |   |-- Normals
|   |   |   |-- Tangents
|   |   |   |-- PolyGroups
|   |   |   |-- Vertex Colors
|   |   |   |-- Material IDs
|   |   |   `-- Mesh Attributes
|   |   |-- UVs
|   |   |   |-- Auto UV
|   |   |   |-- Project UV
|   |   |   |-- Unwrap
|   |   |   |-- Layout
|   |   |   |-- Transform UVs
|   |   |   `-- UV Seams
|   |   |-- Bake
|   |   |   |-- Bake Texture
|   |   |   |-- Bake Normals
|   |   |   |-- Bake Ambient Occlusion
|   |   |   |-- Bake Curvature
|   |   |   |-- Bake Vertex Colors
|   |   |   `-- Bake Multi-Texture
|   |   |-- LOD
|   |   |   |-- Generate LOD
|   |   |   |-- Simplify LOD
|   |   |   |-- Proxy LOD
|   |   |   `-- Mesh LOD Toolset
|   |   |-- Collision
|   |   |   |-- Simple Collision
|   |   |   |-- Convex Decomposition
|   |   |   |-- Mesh Collision
|   |   |   `-- Collision From Mesh
|   |   |-- Volumes
|   |   `-- XForm
|   |-- UV Editor
|   |-- Mesh Paint
|   |-- Geometry Script
|   |-- Dynamic Mesh
|   |-- Geometry Processing
|   |-- Geometry Collections
|   |-- Fracture Mode
|   |   |-- Uniform Voronoi
|   |   |-- Cluster Voronoi
|   |   |-- Radial Voronoi
|   |   |-- Planar Cut
|   |   |-- Slice
|   |   |-- Brick
|   |   |-- Cluster
|   |   |-- Auto Cluster
|   |   |-- Fields
|   |   |-- Anchors
|   |   |-- Removal
|   |   `-- Chaos Destruction
|   |-- Skeletal Mesh Editor
|   |-- Mesh To MetaHuman
|   |-- Proxy Geometry
|   `-- Simplygon
|
|-- 03 Materials / Textures / Shading
|   |-- Material Editor
|   |   |-- Material Graph
|   |   |-- Nodes
|   |   |-- Material Functions
|   |   |-- Material Function Library
|   |   |-- Material Instances
|   |   |-- Material Instance Constants
|   |   |-- Material Parameter Collections
|   |   |-- Material Layers
|   |   |-- Material Layer Blends
|   |   |-- Substrate
|   |   |-- Decal Materials
|   |   |-- Post Process Materials
|   |   |-- Landscape Materials
|   |   |-- Niagara Materials
|   |   |-- UI Materials
|   |   `-- Material Analyzer
|   |-- Texture Editor
|   |   |-- Compression
|   |   |-- Mipmaps
|   |   |-- LOD Bias
|   |   |-- Texture Groups
|   |   |-- sRGB
|   |   |-- Normal Maps
|   |   |-- Virtual Textures
|   |   |-- Texture Streaming
|   |   |-- Texture Arrays
|   |   |-- Volume Textures
|   |   `-- Cube Maps
|   |-- Texture Graph
|   |-- Runtime Virtual Textures
|   |-- Virtual Texture Streaming
|   |-- Render Targets
|   |-- Canvas Render Targets
|   |-- Media Textures
|   |-- Lightmap UV Tools
|   |-- Oodle Texture
|   `-- OpenColorIO
|
|-- 04 Rendering / Lighting / Cinematic Render
|   |-- Renderer Paths
|   |   |-- Deferred Renderer
|   |   |-- Forward Renderer
|   |   |-- Mobile Renderer
|   |   |-- Path Tracer
|   |   `-- Hardware Ray Tracing
|   |-- Micro
|   |   |-- Virtualized Geometry
|   |   |-- Mesh Clusters
|   |   |-- Cluster Streaming
|   |   |-- Micro Foliage
|   |   |-- Micro Voxels
|   |   |-- Micro Assemblies
|   |   |-- Fallback Mesh
|   |   |-- Micro Visualization
|   |   `-- Micro Debug Views
|   |-- Solaris
|   |   |-- Global Illumination
|   |   |-- Reflections
|   |   |-- Surface Cache
|   |   |-- Card Representation
|   |   |-- Screen Traces
|   |   |-- Software Ray Tracing
|   |   |-- Hardware Ray Tracing
|   |   |-- Solaris Scene
|   |   |-- Solaris Visualizations
|   |   `-- Solaris Debug Views
|   |-- Shadows
|   |   |-- Virtual Shadow Maps
|   |   |-- Shadow Maps
|   |   |-- Cascaded Shadow Maps
|   |   |-- Distance Field Shadows
|   |   |-- Contact Shadows
|   |   |-- Capsule Shadows
|   |   |-- Ray Traced Shadows
|   |   `-- Shadow Debug Views
|   |-- Lighting Actors
|   |   |-- Directional Light
|   |   |-- Point Light
|   |   |-- Spot Light
|   |   |-- Rect Light
|   |   |-- Sky Light
|   |   |-- Lightmass Importance Volume
|   |   |-- Lightmass Portal
|   |   |-- Reflection Capture
|   |   `-- Planar Reflection
|   |-- Lighting Systems
|   |   |-- Lightmass
|   |   |-- GPU Lightmass
|   |   |-- MegaLights
|   |   |-- Distance Field AO
|   |   |-- Ambient Occlusion
|   |   |-- IES Profiles
|   |   `-- Light Functions
|   |-- Reflections
|   |   |-- Solaris Reflections
|   |   |-- Screen Space Reflections
|   |   |-- Planar Reflections
|   |   |-- Reflection Captures
|   |   `-- Ray Traced Reflections
|   |-- Post Process
|   |   |-- Post Process Volume
|   |   |-- Tonemapper
|   |   |-- Auto Exposure
|   |   |-- Bloom
|   |   |-- Depth Of Field
|   |   |-- Motion Blur
|   |   |-- Color Grading
|   |   |-- LUT
|   |   |-- Vignette
|   |   |-- Lens Flare
|   |   |-- Chromatic Aberration
|   |   |-- Film Grain
|   |   |-- TSR
|   |   |-- TAA
|   |   |-- FXAA
|   |   |-- MSAA
|   |   `-- Screen Space Effects
|   |-- Render Graph / Shaders
|   |   |-- Render Dependency Graph
|   |   |-- Shader Compile Worker
|   |   |-- Shader Pipeline Cache
|   |   |-- Shader Code Library
|   |   |-- Material Shader Maps
|   |   `-- Derived Data Cache
|   |-- Debug / Views
|   |   |-- Lit
|   |   |-- Unlit
|   |   |-- Wireframe
|   |   |-- Detail Lighting
|   |   |-- Lighting Only
|   |   |-- Light Complexity
|   |   |-- Shader Complexity
|   |   |-- Quad Overdraw
|   |   |-- Micro
|   |   |-- Solaris
|   |   |-- VSM
|   |   |-- GPU Visualizer
|   |   `-- Render Resource Viewer
|   |-- Movie Render Queue
|   |-- Movie Render Graph
|   |-- High Resolution Screenshot
|   |-- Render Targets
|   `-- OpenImageDenoise / OptiX / NNE Denoisers
|
|-- 05 Visual Effects
|   |-- Niagara
|   |   |-- Niagara System Editor
|   |   |-- Niagara Emitter Editor
|   |   |-- Niagara Module Scripts
|   |   |-- Niagara Parameters
|   |   |-- Niagara Data Interfaces
|   |   |-- CPU Sim
|   |   |-- GPU Sim
|   |   |-- Simulation Stages
|   |   |-- Scratch Pad
|   |   |-- User Parameters
|   |   |-- Renderer Modules
|   |   |-- Sprite Renderer
|   |   |-- Mesh Renderer
|   |   |-- Ribbon Renderer
|   |   |-- Light Renderer
|   |   |-- Decal Renderer
|   |   |-- Niagara Fluids
|   |   |-- Niagara Grid 2D
|   |   |-- Niagara Grid 3D
|   |   |-- Niagara Debugger
|   |   |-- Niagara Baker
|   |   `-- Niagara Scalability
|   |-- Cascade Legacy
|   |-- Particle Systems
|   |-- Decals
|   |-- Chaos Fields
|   |-- Volumetric Effects
|   |-- Material VFX
|   `-- Post Process VFX
|
|-- 06 Animation / Characters / MetaHuman
|   |-- Animation Editors
|   |   |-- Skeleton Editor
|   |   |-- Skeletal Mesh Editor
|   |   |-- Animation Sequence Editor
|   |   |-- Animation Blueprint Editor
|   |   `-- Animation Mode
|   |-- Skeleton
|   |   |-- Bone Tree
|   |   |-- Retarget Sources
|   |   |-- Sockets
|   |   |-- Virtual Bones
|   |   `-- Preview Mesh
|   |-- Skeletal Mesh
|   |   |-- Materials
|   |   |-- LODs
|   |   |-- Morph Targets
|   |   |-- Clothing
|   |   |-- Physics Asset
|   |   |-- Skin Weights
|   |   `-- Mesh Editing Tools
|   |-- Animation Assets
|   |   |-- Animation Sequence
|   |   |-- Animation Composite
|   |   |-- Animation Montage
|   |   |-- Blend Space
|   |   |-- Aim Offset
|   |   |-- Pose Asset
|   |   |-- Notifies
|   |   |-- Curves
|   |   `-- Sync Markers
|   |-- Animation Blueprint
|   |   |-- Anim Graph
|   |   |-- Event Graph
|   |   |-- State Machines
|   |   |-- Transition Rules
|   |   |-- Blend Nodes
|   |   |-- IK Nodes
|   |   |-- Cached Poses
|   |   |-- Linked Anim Graphs
|   |   `-- Animation Debugger
|   |-- Control Rig
|   |   |-- Rig Graph
|   |   |-- RigVM
|   |   |-- Rig Hierarchy
|   |   |-- Controls
|   |   |-- Spaces
|   |   |-- Constraints
|   |   |-- Full Body IK
|   |   |-- Forward Solve
|   |   |-- Backward Solve
|   |   `-- Control Rig Sequencer Tracks
|   |-- IK
|   |   |-- IK Rig
|   |   |-- IK Retargeter
|   |   |-- Full Body IK
|   |   |-- FABRIK
|   |   |-- CCDIK
|   |   `-- Two Bone IK
|   |-- Motion
|   |   |-- Motion Matching
|   |   |-- Pose Search
|   |   |-- Motion Warping
|   |   |-- Distance Matching
|   |   |-- Root Motion
|   |   `-- Trajectory Tools
|   |-- Deformation
|   |   |-- Deformer Graph
|   |   |-- ML Deformer
|   |   |-- Morph Targets
|   |   |-- Cloth Paint
|   |   |-- Chaos Cloth
|   |   `-- Optimus
|   |-- Groom / Hair
|   |   |-- Groom Asset Editor
|   |   |-- Hair Strands
|   |   |-- Groom Binding
|   |   |-- Cards
|   |   |-- Meshes
|   |   `-- Hair Simulation
|   |-- MetaHuman
|   |   |-- MetaHuman Creator
|   |   |-- MetaHuman Animator
|   |   |-- MetaHuman Live Link
|   |   |-- MetaHuman SDK
|   |   |-- RigLogic
|   |   `-- Mesh To MetaHuman
|   `-- Animation Budget Allocator
|
|-- 07 Cinematics / Virtual Production
|   |-- Sequencer
|   |   |-- Level Sequence
|   |   |-- Master Sequence
|   |   |-- Tracks
|   |   |-- Sections
|   |   |-- Keyframes
|   |   |-- Curve Editor
|   |   |-- Cameras
|   |   |-- Camera Cuts
|   |   |-- Event Tracks
|   |   |-- Spawnables
|   |   |-- Possessables
|   |   |-- Sub-Sequences
|   |   |-- Control Rig Tracks
|   |   |-- Audio Tracks
|   |   `-- Render Movie
|   |-- Cameras
|   |   |-- Cine Camera Actor
|   |   |-- Camera Rig Rail
|   |   |-- Camera Rig Crane
|   |   |-- Camera Shake
|   |   |-- Gameplay Cameras
|   |   `-- Camera Calibration
|   |-- Recording
|   |   |-- Take Recorder
|   |   |-- Sequence Recorder Legacy
|   |   |-- Live Link Recorder
|   |   `-- Performance Capture
|   |-- Rendering
|   |   |-- Movie Render Queue
|   |   |-- Movie Render Graph
|   |   |-- Path Tracer
|   |   |-- EXR
|   |   |-- ProRes
|   |   `-- Burn In
|   |-- Virtual Production
|   |   |-- Live Link
|   |   |-- nDisplay
|   |   |-- Switchboard
|   |   |-- Multi-User Editing
|   |   |-- ICVFX
|   |   |-- Composure
|   |   |-- OpenColorIO
|   |   |-- DMX
|   |   |-- Remote Control
|   |   |-- Virtual Camera
|   |   |-- Stage App
|   |   `-- LED Wall Tools
|   `-- Media Framework
|       |-- Media Player
|       |-- Media Plate
|       |-- Media Texture
|       |-- Img Media
|       |-- Electra Player
|       `-- Capture Cards
|
|-- 08 Gameplay / Scripting / Logic
|   |-- Blueprint System
|   |   |-- Blueprint Editor
|   |   |-- Event Graph
|   |   |-- Construction Script
|   |   |-- Functions
|   |   |-- Macros
|   |   |-- Variables
|   |   |-- Components
|   |   |-- Timelines
|   |   |-- Interfaces
|   |   |-- Blueprint Function Libraries
|   |   |-- Blueprint Macro Libraries
|   |   |-- Blueprint Debugger
|   |   |-- Blueprint Diff
|   |   |-- Blueprint Nativization Legacy
|   |   `-- Blueprint Stats
|   |-- C++ Gameplay
|   |   |-- Classes
|   |   |-- Modules
|   |   |-- Components
|   |   |-- Subsystems
|   |   |-- UObject
|   |   |-- UCLASS
|   |   |-- USTRUCT
|   |   |-- UFUNCTION
|   |   `-- UPROPERTY
|   |-- Gameplay Framework
|   |   |-- Actor
|   |   |-- Pawn
|   |   |-- Character
|   |   |-- PlayerController
|   |   |-- AIController
|   |   |-- GameMode
|   |   |-- GameState
|   |   |-- PlayerState
|   |   |-- HUD
|   |   |-- GameInstance
|   |   |-- GameFeature
|   |   `-- Subsystems
|   |-- Input
|   |   |-- Enhanced Input
|   |   |-- Input Actions
|   |   |-- Input Mapping Contexts
|   |   |-- Triggers
|   |   |-- Modifiers
|   |   `-- Common Input
|   |-- Systems
|   |   |-- Gameplay Ability System
|   |   |-- Gameplay Tags
|   |   |-- Gameplay Tasks
|   |   |-- StateTree
|   |   |-- Smart Objects
|   |   |-- Chooser
|   |   |-- Data Assets
|   |   |-- Data Tables
|   |   |-- Curves
|   |   |-- Save Game
|   |   `-- Modular Gameplay
|   `-- Scripting / Automation
|       |-- Python
|       |-- Editor Utility Widgets
|       |-- Editor Utility Blueprints
|       |-- Blutilities
|       |-- Commandlets
|       `-- Remote Control API
|
|-- 09 AI / Navigation / Crowds
|   |-- Behavior Trees
|   |   |-- Behavior Tree Editor
|   |   |-- Tasks
|   |   |-- Decorators
|   |   |-- Services
|   |   `-- Composites
|   |-- Blackboard
|   |   |-- Blackboard Editor
|   |   |-- Keys
|   |   `-- Blackboard Components
|   |-- EQS
|   |   |-- Environment Query Editor
|   |   |-- Generators
|   |   |-- Tests
|   |   |-- Contexts
|   |   `-- Debugger
|   |-- Navigation
|   |   |-- NavMesh Bounds Volume
|   |   |-- Recast NavMesh
|   |   |-- Nav Areas
|   |   |-- Nav Links
|   |   |-- Smart Links
|   |   |-- Navigation Invokers
|   |   `-- Navigation Debugger
|   |-- Perception
|   |   |-- Sight
|   |   |-- Hearing
|   |   |-- Damage
|   |   |-- Touch
|   |   |-- Prediction
|   |   `-- Team
|   |-- Mass
|   |   |-- Mass Entity
|   |   |-- Mass AI
|   |   |-- Mass Crowd
|   |   |-- Processors
|   |   |-- Fragments
|   |   |-- Archetypes
|   |   `-- ZoneGraph
|   |-- Learning Agents
|   |-- ML Adapter
|   |-- HTN Planner
|   `-- Gameplay Debugger
|
|-- 10 Physics / Destruction / Vehicles
|   |-- Chaos Physics
|   |   |-- Rigid Bodies
|   |   |-- Collision
|   |   |-- Constraints
|   |   |-- Physics Materials
|   |   |-- Physical Animation
|   |   |-- Physics Fields
|   |   |-- Solvers
|   |   `-- Substepping
|   |-- Physics Asset Editor
|   |   |-- Bodies
|   |   |-- Constraints
|   |   |-- Profiles
|   |   |-- Simulation
|   |   `-- Collision Primitives
|   |-- Destruction
|   |   |-- Geometry Collections
|   |   |-- Fracture Mode
|   |   |-- Chaos Fields
|   |   |-- Clustering
|   |   |-- Damage
|   |   `-- Cache Manager
|   |-- Vehicles
|   |   |-- Chaos Vehicles
|   |   |-- Wheeled Vehicle Pawn
|   |   |-- Vehicle Movement Component
|   |   |-- Wheel Blueprints
|   |   `-- Vehicle Debug
|   |-- Cloth
|   |   |-- Chaos Cloth
|   |   |-- Cloth Asset Editor
|   |   |-- Paint Weights
|   |   |-- Simulation
|   |   `-- LODs
|   |-- Fluids
|   |-- Cable Component
|   |-- Buoyancy
|   `-- Water Physics
|
|-- 11 Audio
|   |-- Sound Wave Editor
|   |-- Sound Cue Editor
|   |-- Audio Mixer
|   |-- MetaSounds
|   |   |-- MetaSound Source
|   |   |-- MetaSound Patch
|   |   |-- Graph
|   |   |-- Inputs
|   |   |-- Outputs
|   |   |-- Parameters
|   |   `-- Presets
|   |-- Quartz
|   |-- Audio Modulation
|   |-- Audio Synesthesia
|   |-- Submixes
|   |-- Sound Attenuation
|   |-- Sound Concurrency
|   |-- Reverb
|   |-- Audio Volumes
|   |-- Soundscape
|   |-- MotoSynth
|   |-- Harmonix
|   `-- Waveform Editor
|
|-- 12 UI / UX
|   |-- UMG
|   |   |-- Widget Blueprint Editor
|   |   |-- Designer
|   |   |-- Graph
|   |   |-- Palette
|   |   |-- Hierarchy
|   |   |-- Bindings
|   |   |-- Animations
|   |   |-- User Widgets
|   |   `-- Widget Components
|   |-- Slate
|   |-- Common UI
|   |-- Common Input
|   |-- UI Materials
|   |-- Retainer Box
|   |-- Invalidation Panel
|   |-- HUD
|   `-- Debug UI
|
|-- 13 Networking / Online / Multiplayer
|   |-- Replication
|   |   |-- Actors
|   |   |-- Properties
|   |   |-- RPCs
|   |   |-- Relevancy
|   |   |-- Dormancy
|   |   `-- Replication Conditions
|   |-- Iris
|   |-- Replication Graph
|   |-- Network Prediction
|   |-- Network Profiler
|   |-- Online Subsystem
|   |-- Online Services
|   |-- Epic Online Services
|   |-- Sessions
|   |-- Lobbies
|   |-- Matchmaking
|   |-- Achievements
|   |-- Leaderboards
|   |-- Voice Chat
|   |-- Steam Sockets
|   |-- WebSockets
|   |-- Pixel Streaming
|   `-- Multiplayer PIE
|
|-- 14 Import / Export / DCC Pipeline
|   |-- Interchange
|   |-- FBX Importer
|   |-- glTF Importer / Exporter
|   |-- USD
|   |-- Datasmith
|   |-- Dataprep
|   |-- Alembic
|   |-- Geometry Cache
|   |-- Groom Import
|   |-- Media Import
|   |-- Texture Import
|   |-- Reimport
|   |-- Asset Migration
|   |-- Bulk Property Matrix
|   |-- Asset Registry
|   |-- Data Validation
|   |-- DDC
|   |-- Bridge Legacy
|   `-- Fab
|
|-- 15 Build / Cook / Package / Deploy
|   |-- UnrealBuildTool
|   |-- UnrealHeaderTool
|   |-- Unreal Automation Tool
|   |-- BuildGraph
|   |-- Cooker
|   |-- Project Launcher
|   |-- Packaging Settings
|   |-- Pak Files
|   |-- IoStore
|   |-- Chunking
|   |-- Localization Gather
|   |-- Device Manager
|   |-- Device Profiles
|   |-- Platform SDKs
|   |-- Crash Reporter
|   |-- Automation Tests
|   |-- Gauntlet
|   |-- Functional Testing
|   `-- Data Validation Commandlets
|
|-- 16 Profiling / Debugging / Optimization
|   |-- Unreal Insights
|   |   |-- Timing Insights
|   |   |-- Memory Insights
|   |   |-- Networking Insights
|   |   |-- Asset Loading Insights
|   |   |-- Animation Insights
|   |   |-- Trace Store
|   |   `-- Trace Server
|   |-- Session Frontend
|   |-- Frontend Profiler
|   |-- Stat Commands
|   |-- CSV Profiler
|   |-- GPU Visualizer
|   |-- RenderDoc Integration
|   |-- Render Resource Viewer
|   |-- Shader Complexity
|   |-- Quad Overdraw
|   |-- Light Complexity
|   |-- Micro Debug
|   |-- Solaris Debug
|   |-- VSM Debug
|   |-- Collision Analyzer
|   |-- Visual Logger
|   |-- Gameplay Debugger
|   |-- Blueprint Debugger
|   |-- Niagara Debugger
|   |-- Animation Debugger
|   |-- Memory Report
|   |-- Low Level Memory Tracker
|   |-- Asset Audit
|   `-- Reference Viewer
|
|-- 17 Platform / Device / XR
|   |-- Windows
|   |-- macOS
|   |-- Linux
|   |-- Android
|   |-- iOS
|   |-- PlayStation
|   |-- Xbox
|   |-- Nintendo Switch
|   |-- Mobile Previewer
|   |-- Device Output Log
|   |-- Device Profiles
|   |-- Scalability
|   |-- XR
|   |   |-- OpenXR
|   |   |-- VR Template
|   |   |-- AR Template
|   |   |-- Motion Controllers
|   |   |-- Hand Tracking
|   |   |-- Eye Tracking
|   |   |-- Mixed Reality Capture
|   |   |-- HMD Plugins
|   |   `-- XR Visualization
|   |-- ARKit
|   |-- ARCore
|   |-- Oculus / MetaXR
|   |-- SteamVR
|   `-- OpenXR Extensions
|
|-- 18 Built-In Plugin Families
|   |-- 2D
|   |   `-- Paper2D
|   |-- Accessibility
|   |   |-- ScreenReader
|   |   |-- SlateScreenReader
|   |   `-- TextToSpeech
|   |-- Advertising
|   |   `-- IOS TapJoy Advertising Provider
|   |-- AI
|   |   |-- AISupport
|   |   |-- Environment Query Editor
|   |   |-- HTN Planner
|   |   |-- Learning Agents
|   |   |-- MassAI
|   |   |-- MassCrowd
|   |   |-- ML Adapter
|   |   |-- ZoneGraph
|   |   `-- ZoneGraph Annotations
|   |-- Analytics
|   |   |-- Adjust Analytics Provider
|   |   |-- Analytics Blueprint Library
|   |   |-- File Logging Analytics Provider
|   |   `-- Multicast Analytics Provider
|   |-- Android
|   |   |-- AndroidFileServer
|   |   |-- Android Background Service
|   |   `-- GooglePAD
|   |-- Animation
|   |   |-- Animation Budget Allocator
|   |   |-- Animation Compression Library
|   |   |-- Animation Curve Expressions
|   |   |-- Animation Data
|   |   |-- Live Link Hub Example Device
|   |   |-- Live Link Hub Unreal Device
|   |   |-- Locomotor
|   |   |-- ML Deformer Framework
|   |   |-- ML Deformer Detail Pose Model
|   |   |-- ML Deformer Neural Morph Model
|   |   |-- ML Deformer Vertex Delta Model
|   |   |-- Motion Trajectory
|   |   |-- Motion Warping
|   |   |-- Movie Scene Pose Search Tracks
|   |   |-- Mutable Dataflow Extensions
|   |   |-- Mutable Groom Extensions
|   |   |-- Optimus
|   |   |-- Packed Attributes
|   |   |-- Performance Capture Core
|   |   |-- Performance Capture Workflow
|   |   |-- Pose Search
|   |   |-- Relative IK Op
|   |   |-- Rig Mapper
|   |   |-- RigLogic for UAF
|   |   |-- Sequence Navigator
|   |   |-- Sequencer Anim Mixer
|   |   |-- Skeletal Mesh Editing Tools
|   |   |-- Skeletal Mesh Morph Target Editing Tools
|   |   |-- Trajectory Tools
|   |   |-- Tweening Utils
|   |   `-- Unreal Animation Framework
|   |-- Audio
|   |   |-- Audio Capture
|   |   |-- Audio Definition Model
|   |   |-- Audio Insights
|   |   |-- Audio Modulation
|   |   |-- Audio Motor Sim
|   |   |-- Audio Synesthesia
|   |   |-- AudioGameplay
|   |   |-- AudioGameplayVolume
|   |   |-- AudioWidgets
|   |   |-- Harmonix
|   |   |-- MetaSound
|   |   |-- MetaSounds Experimental
|   |   |-- MicrosoftSpatialSound
|   |   |-- MotoSynth
|   |   |-- Music Environment
|   |   |-- Sound Cue Templates
|   |   |-- Sound Utilities
|   |   |-- SoundFields
|   |   |-- Soundscape Plugin
|   |   |-- Spatialization
|   |   |-- Subtitles and Closed Captions
|   |   |-- Synthesis and DSP Effects
|   |   |-- TechAudioTools
|   |   |-- Wave Tables
|   |   `-- Waveform Editor
|   |-- Augmented Reality
|   |   |-- Apple ARKit
|   |   |-- Apple ARKit Face Support
|   |   `-- AR Utilities
|   |-- BlendSpace
|   |   `-- Blendspace Motion Analysis
|   |-- Blueprints
|   |   |-- Blueprint C++ Header Preview
|   |   |-- Blueprint File Utilities
|   |   |-- Blueprint Stats
|   |   |-- Json Blueprint Utilities
|   |   `-- Property Access Node
|   |-- Build Distribution
|   |   |-- FastBuild Controller
|   |   |-- UBA Controller
|   |   `-- XGE Controller
|   |-- Cameras
|   |   |-- Camera Shake Previewer
|   |   |-- Engine Cameras
|   |   `-- Gameplay Cameras
|   |-- Codecs
|   |   |-- AMFCodecs
|   |   |-- AVCodecs Core
|   |   |-- LibVpxCodecs
|   |   |-- NVCodecs
|   |   |-- VTCodecs
|   |   `-- WMFCodecs
|   |-- Compositing
|   |   |-- CompositeCore
|   |   |-- Composure
|   |   |-- HoldoutComposite
|   |   |-- Legacy Composure
|   |   |-- Lens Distortion Deprecated
|   |   `-- OpenCV Lens Distortion
|   |-- Compression
|   |   |-- Oodle Network
|   |   `-- Oodle Texture
|   |-- Computer Vision
|   |   `-- OpenCV
|   |-- Content Browser
|   |   |-- Alias Data Source
|   |   |-- Asset Data Source
|   |   |-- Class Data Source
|   |   `-- File Data Source
|   |-- Customizable Objects
|   |   |-- Mutable
|   |   `-- Mutable Population
|   |-- Database
|   |   |-- ADO Support
|   |   |-- Database Support
|   |   |-- Remote Database Support
|   |   |-- SQLite
|   |   `-- SQLite Support
|   |-- Dataprep
|   |   |-- Dataprep Editor
|   |   `-- Dataprep Geometry Operations
|   |-- Denoising
|   |   |-- NFORDenoise
|   |   |-- NNEDenoiser
|   |   |-- OpenImageDenoise
|   |   `-- OptiXDenoise
|   |-- Device Profiles
|   |   |-- Android Device Profile Selector
|   |   |-- Example Device Profile Selector
|   |   |-- IOS Device Profile Selector
|   |   |-- Linux Device Profile Selector
|   |   `-- Windows Device Profile Selector
|   |-- Editor
|   |   |-- Actor Layer Utilities
|   |   |-- Actor Sequence
|   |   |-- Asset Manager Editor
|   |   |-- Asset Referencing Restrictions
|   |   |-- Asset Registry Export
|   |   |-- Asset Search
|   |   |-- Bridge
|   |   |-- ChaosEditor
|   |   |-- Curve Editor Tools
|   |   |-- Data Validation
|   |   |-- Editor DataflowGraph
|   |   |-- Facial Animation Bulk Importer
|   |   |-- Flow Production Tracking
|   |   |-- GameplayTagsEditor
|   |   |-- GeometryMode
|   |   |-- GPU Lightmass
|   |   |-- InstanceDataObject Fixup Tool
|   |   |-- Landscape Patch
|   |   |-- Level Sequence Editor
|   |   |-- Light Mixer
|   |   |-- Material Analyzer
|   |   |-- Media Player Editor
|   |   |-- Mesh Painting
|   |   |-- Mesh Resizing
|   |   |-- Modeling Tools Editor Mode
|   |   |-- Multi-User Editing
|   |   |-- Object Mixer
|   |   |-- Plugin Audit
|   |   |-- Plugin Browser
|   |   |-- Plugin Reference Viewer
|   |   |-- Plugin Template Tool
|   |   |-- PCG Framework
|   |   |-- PCG External Data Interop
|   |   |-- PCG FastGeo Interop
|   |   |-- PCG Geometry Script Interop
|   |   |-- PCG Instanced Actors Interop
|   |   |-- PCG Micro Assemblies Interop
|   |   |-- PCG Niagara Interop
|   |   |-- PCG Python Interop
|   |   |-- PCG Water Interop
|   |   |-- Proxy LOD
|   |   |-- Recovery Hub
|   |   |-- Sample Tools Editor Mode
|   |   |-- Scriptable Tools Editor Mode
|   |   |-- Scriptable Tools Framework
|   |   |-- Sequence Validator
|   |   |-- Sequencer Anim Tools
|   |   |-- Skeletal Mesh Simplifier
|   |   |-- Static Mesh Editor Modeling Mode
|   |   |-- TEDS Editor Data Storage
|   |   |-- TextureGraph
|   |   |-- Tool Palette For Widget Editor
|   |   |-- Tool Presets
|   |   |-- UserToolBox
|   |   |-- Variant Manager
|   |   |-- Workspace
|   |   `-- World Partition HLOD Utilities
|   |-- MetaHuman
|   |   |-- MetaHuman Animator
|   |   |-- MetaHuman Animator Calibration Diagnostics
|   |   |-- MetaHuman Animator Calibration Processing
|   |   |-- MetaHuman Core Tech
|   |   |-- MetaHuman Creator
|   |   |-- MetaHuman Creator UAF Support
|   |   |-- MetaHuman Live Link
|   |   `-- MetaHuman SDK
|   |-- Misc
|   |   |-- Color Grading
|   |   |-- Data Charts
|   |   |-- DMX Modular Features
|   |   |-- Live Link Over nDisplay
|   |   |-- Mac Graphics Switching
|   |   |-- nDisplay
|   |   |-- Platform Cryptography
|   |   |-- RigVM
|   |   |-- Sun Position Calculator
|   |   |-- Voice Chat Interface
|   |   `-- Web Authentication
|   |-- Other
|   |   |-- Chaos Cloth Asset Editor
|   |   |-- Color Correction Regions
|   |   |-- Dataflow
|   |   |-- Day Sequence
|   |   |-- Engine Asset Definitions
|   |   |-- Fab
|   |   |-- Full Body IK
|   |   |-- GeoReferencing
|   |   |-- Image Widgets
|   |   |-- Landmass
|   |   |-- LiveLinkFaceImporter
|   |   |-- Media Viewer
|   |   |-- Mesh LOD Toolset
|   |   |-- Mesh Modeling Toolset
|   |   |-- Mover
|   |   |-- Niagara Example Custom DataInterface
|   |   |-- PCG Biome Core
|   |   |-- PCG Biome Sample
|   |   |-- Post Process Material Chain Graph
|   |   |-- Skeletal Merging
|   |   |-- State Graph
|   |   |-- SurfaceEffects
|   |   |-- USD Core
|   |   `-- UVEditor
|   |-- Virtual Reality
|   |   |-- Mixed Reality Capture Framework
|   |   |-- OpenXR
|   |   |-- OpenXREyeTracker
|   |   |-- OpenXRHandTracking
|   |   |-- OpenXRMsftHandInteraction
|   |   |-- OpenXRViveTracker
|   |   |-- Sample Mesh Reconstructor
|   |   |-- SimpleHMD
|   |   |-- XRBase
|   |   `-- XRScribe
|   |-- Water
|   |   |-- Buoyancy
|   |   |-- Water
|   |   |-- Water Advanced
|   |   `-- Water Extras
|   |-- Web
|   |   |-- HttpBlueprint
|   |   `-- WebAPI
|   `-- World Building
|       `-- FastGeo Streaming
|   |-- Encoders
|   |   `-- HardwareEncoders
|   |-- Engine
|   |   `-- MemoryUsageQueries
|   |-- Examples
|   |   |-- Blank Example Plugin
|   |   |-- Script Plugin
|   |   `-- UObject Example Plugin
|   |-- Experimental
|   |   |-- Actor Palette
|   |   |-- Apple Image Utils
|   |   |-- BackChannel
|   |   |-- Batch Renamer
|   |   |-- Blueprint Snap Nodes Prototype
|   |   |-- Floating Properties
|   |   |-- Gizmo Editor Mode
|   |   |-- Media Stream
|   |   |-- Motion Design For nDisplay
|   |   |-- Neural Rendering
|   |   |-- New TRS Gizmos
|   |   |-- NNERuntimeBasicCpu
|   |   |-- Render Grid
|   |   |-- SVG Importer
|   |   |-- Wave Function Collapse
|   |   `-- Web Socket Messaging
|   |-- Exporters
|   |   `-- glTF Exporter
|   |-- Framework
|   |   |-- AsyncMessageSystem
|   |   `-- AsyncMessageSystemTests
|   |-- FX
|   |   |-- Cascade Editor
|   |   |-- Cascade To Niagara Converter
|   |   |-- Niagara
|   |   |-- Niagara MRQ Support
|   |   |-- Niagara Micro
|   |   |-- NiagaraFluids
|   |   |-- NiagaraPreviewContent
|   |   `-- NiagaraSimCaching
|   |-- Gameplay
|   |   |-- AI Behaviors
|   |   |-- ArchVis Character
|   |   |-- CharacterAI
|   |   |-- Common Conversation
|   |   |-- Data Registry
|   |   |-- Draw Debug Library
|   |   |-- Game Features
|   |   |-- Gameplay Abilities
|   |   |-- Gameplay Abilities Game Feature Actions
|   |   |-- Gameplay Graph
|   |   |-- GameplayInteractions
|   |   |-- GameplayStateTree
|   |   |-- InstancedActors
|   |   |-- InteractionInterface
|   |   |-- MassEntity
|   |   |-- MassGameplay
|   |   |-- Modular Gameplay
|   |   |-- Mover
|   |   |-- Network Prediction
|   |   |-- SmartObjects
|   |   |-- StateTree
|   |   `-- Targeting System
|   |-- Gameplay Streaming
|   |   `-- NVidia GeForce NOW Wrapper
|   |-- Geometry
|   |   |-- Alembic Groom Importer
|   |   |-- Field System
|   |   |-- Geometry
|   |   |-- Geometry Dataflow Nodes
|   |   |-- Geometry Processing
|   |   |-- Geometry Script
|   |   |-- GeometryFlow
|   |   |-- Groom
|   |   |-- Hair Card Generator
|   |   `-- Hair Modeling Toolset
|   |-- Graphics
|   |   |-- Pixel Capture
|   |   |-- Pixel Streaming
|   |   |-- Pixel Streaming 2
|   |   `-- Pixel Streaming Player
|   |-- Importers
|   |   |-- Alembic Importer
|   |   |-- AxF Importer
|   |   |-- Chaos Caching USD
|   |   |-- Datasmith C4D Importer
|   |   |-- Datasmith CAD Importer
|   |   |-- Datasmith Content
|   |   |-- Datasmith FBX Importer
|   |   |-- Datasmith Importer
|   |   |-- Datasmith Interchange
|   |   |-- Geometry Cache
|   |   |-- Interchange Editor
|   |   |-- Interchange Framework
|   |   |-- Interchange OpenUSD
|   |   |-- Interchange OpenVDB
|   |   |-- MDL Importer
|   |   |-- USD Importer
|   |   |-- USD Importer MDL Integration
|   |   `-- USD Multi-User Synchronization
|   |-- Input
|   |   |-- Enhanced Input
|   |   `-- Input Debugging
|   |-- Input Devices
|   |   |-- Game Input Windows
|   |   |-- Game Input Base
|   |   |-- MIDI Device Support
|   |   |-- OSC
|   |   |-- Steam Controller Plugin
|   |   |-- Stylus And Tablet Plugin
|   |   |-- Windows DualShock
|   |   |-- Windows RawInput
|   |   |-- Windows Virtual Keyboard
|   |   `-- XInput Device
|   |-- Insights
|   |   |-- Animation Insights
|   |   |-- Chaos Insights
|   |   |-- Insights Data Source Filters
|   |   |-- IoStore Insights
|   |   |-- Mass Insights
|   |   |-- Network Prediction Insights
|   |   |-- RDG Insights
|   |   `-- Slate Insights
|   |-- IOT
|   |   `-- MQTT
|   |-- Learning
|   |   |-- Guided Tutorials
|   |   `-- In-Editor Documentation
|   |-- Localization
|   |   |-- OneSky
|   |   `-- Portable Object File Data Source
|   |-- Media
|   |   `-- Media Plate
|   |-- Media Players
|   |   |-- AJA Media Player
|   |   |-- Android Camera Player
|   |   |-- Apple ProRes Media
|   |   |-- AVF Media Player
|   |   |-- Avid DNxHR/DNxMXF Media Plugin
|   |   |-- Bink Media
|   |   |-- Blackmagic Media Player
|   |   |-- Electra Codecs
|   |   |-- Electra Player
|   |   |-- HAP Media
|   |   |-- Image Sequence Media Player
|   |   |-- Media Framework Utilities
|   |   |-- NDI Media
|   |   |-- WebM Video Player
|   |   |-- Windows Movie Player
|   |   `-- WMF Media Player
|   |-- Mesh
|   |   `-- ImpostorBaker
|   |-- Messaging
|   |   |-- Discovery Beacon Receiver
|   |   |-- Epic Stage App
|   |   |-- Localizable Message
|   |   |-- Messaging Debugger
|   |   |-- QUIC Messaging
|   |   |-- Remote Control API
|   |   |-- Remote Control Web Interface
|   |   |-- TCP Messaging
|   |   `-- UDP Messaging
|   |-- ML
|   |   |-- NNERuntimeCoreML
|   |   |-- NNERuntimeIREE
|   |   |-- NNERuntimeORT
|   |   `-- NNERuntimeRDG
|   |-- Mobile
|   |   |-- Mobile Location Services Android
|   |   |-- Mobile Location Services iOS
|   |   |-- Mobile Location Services Blueprints Library
|   |   |-- Mobile Patching Utilities
|   |   |-- Optional Mobile Features Blueprint Library
|   |   `-- ReplayKit for iOS
|   |-- Networking
|   |   |-- Concert Main
|   |   |-- Concert Sync Client
|   |   |-- Concert Sync Server
|   |   |-- Iris
|   |   |-- Multi User Server
|   |   |-- Multi-server Replication
|   |   |-- Netcode Unit Test
|   |   |-- Replication System Test Plugin
|   |   `-- Steam Sockets
|   |-- Online
|   |   `-- Online Framework Common
|   |-- Online Platform
|   |   |-- Chunk Downloader
|   |   |-- EOS Overlay Input Provider
|   |   |-- EOS Shared
|   |   |-- EOS Voice Chat
|   |   |-- Firebase
|   |   |-- Google Cloud Messaging
|   |   |-- Online Services
|   |   |-- Online Services EOS
|   |   |-- Online Services Null
|   |   |-- Online Subsystem
|   |   |-- Online Subsystem EOS
|   |   |-- Online Subsystem Steam
|   |   |-- Online Subsystem Utils
|   |   |-- Socket Subsystem EOS
|   |   `-- Voice Chat Interface
|   |-- Performance
|   |   |-- Editor Performance
|   |   |-- PerformanceMonitor
|   |   |-- Reflex
|   |   |-- Replication Graph
|   |   `-- Significance Manager
|   |-- Peripherals
|   |   `-- Razer Chroma Devices
|   |-- Platform
|   |   `-- Project Launcher
|   |-- Profiling
|   |   |-- Low-level network trace Plugin
|   |   `-- World Metrics
|   |-- Programming
|   |   |-- 10X Editor Integration
|   |   |-- Code Editor
|   |   |-- Code View
|   |   |-- KDevelop Integration
|   |   |-- Plugin Utilities
|   |   |-- Visual Studio Code Integration
|   |   |-- Visual Studio Integration
|   |   `-- XCode Integration
|   |-- Rendering
|   |   |-- Blueprint Material and Texture Nodes
|   |   |-- Cable Component
|   |   |-- Cinematic Prestreaming
|   |   |-- Compute Framework
|   |   |-- Custom Mesh Component
|   |   |-- Dump GPU Services
|   |   |-- Dynamic Wind
|   |   |-- External GPU Statistics
|   |   |-- GPU Reshape Plugin
|   |   |-- HDRIBackdrop
|   |   |-- LiDAR Point Cloud Support
|   |   |-- Movie Render Queue
|   |   |-- Movie Render Queue Additional Render Passes
|   |   |-- Micro Assembly Editor Utilities
|   |   |-- Micro Displaced Mesh
|   |   |-- OpenColorIO
|   |   |-- PIX On Windows GPU Capture Plugin
|   |   |-- Procedural Mesh Component
|   |   |-- Virtual Heightfield Mesh
|   |   `-- Volumetrics
|   |-- Runtime
|   |   |-- Customizable Sequencer Tracks
|   |   |-- MsQuic Runtime Plugin
|   |   `-- Property Access Editor
|   |-- Scripting
|   |   |-- Editor Scripting Utilities
|   |   |-- MLflow
|   |   |-- Python Editor Script Plugin
|   |   |-- Python Foundation Packages
|   |   |-- Python ML Package
|   |   |-- Sequencer Scripting
|   |   |-- Slate Scripting
|   |   |-- TargetDeviceServices scripting library
|   |   `-- Tensorboard
|   |-- Security
|   |   `-- Security Sandbox
|   |-- Source Control
|   |   |-- Changelist Reviews
|   |   |-- Directory Placeholder
|   |   |-- Git
|   |   |-- Perforce
|   |   `-- Subversion
|   |-- Telemetry
|   |   `-- Editor Telemetry
|   |-- Testing
|   |   |-- Audio Code Quality Tests
|   |   |-- Automation Driver Tests
|   |   |-- Automation Utilities
|   |   |-- Editor Tests
|   |   |-- Functional Testing Editor
|   |   |-- Gauntlet
|   |   |-- Python Automation Test
|   |   |-- RHI Tests
|   |   |-- Runtime Tests
|   |   |-- TestFramework
|   |   `-- WidgetAutomationTests
|   |-- Text
|   |   |-- MovieSceneTextTrack
|   |   `-- Text 3D
|   |-- UI
|   |   |-- Common UI Plugin
|   |   |-- Slate Model View Viewmodel
|   |   |-- SlateIM
|   |   |-- UIFramework
|   |   |-- UMG Viewmodel
|   |   |-- UMG Widget Preview
|   |   |-- Web Browser
|   |   `-- Web Browser to Native Proxying
|   |-- Virtual Production
|   |   |-- Actor Modifier
|   |   |-- Camera Calibration
|   |   |-- Capture Manager
|   |   |-- CineCameraRigs
|   |   |-- Cinematic Assembly Tools
|   |   |-- Cloners and Effectors
|   |   |-- Composure
|   |   |-- Console Variables Editor
|   |   |-- DMX Control Console
|   |   |-- ICVFX
|   |   |-- LiveLinkCamera
|   |   |-- LiveLinkXR
|   |   |-- Material Designer
|   |   |-- Media IO Framework
|   |   |-- Motion Design
|   |   |-- nDisplay Launch
|   |   |-- NVIDIA Rivermax
|   |   |-- Stage Monitor
|   |   |-- Switchboard
|   |   |-- Take Recorder
|   |   |-- Texture Share
|   |   |-- VirtualCamera
|   |   `-- XR Creative Framework
|
`-- 19 External / Indie Plugin Ecosystem Verified Examples
    |-- World / Terrain / Voxel
    |   |-- Voxel Plugin
    |   |-- Voxy
    |   |-- Dungeon Architect
    |   |-- Procedural Dungeon
    |   |-- Brushify
    |   |-- Errant Landscape
    |   |-- Easy Mapper
    |   |-- WorldScape
    |   |-- Oceanology
    |   |-- Fluid Flux
    |   |-- Fluid Ninja
    |   |-- Ultra Dynamic Sky
    |   |-- Sky Creator
    |   |-- Infinity Weather
    |   `-- Dynamic Weather / Seasons Packs
    |-- Procedural / Tools / DCC
    |   |-- Houdini Engine For Unreal
    |   |-- SideFX Labs Unreal Tools
    |   |-- Substance 3D Plugin
    |   |-- InstaLOD
    |   |-- Simplygon External
    |   |-- Auto Setup For Character Creator
    |   |-- Reallusion Live Link
    |   |-- Mesh Morpher
    |   |-- Runtime Mesh Component
    |   |-- Realtime Mesh Component
    |   |-- Runtime Spline Builder
    |   `-- Blueprint Assist
    |-- Gameplay Frameworks
    |   |-- Logic Driver Pro
    |   |-- Able Ability System
    |   |-- GAS Companion
    |   |-- Easy Multi Save
    |   |-- Savior Save System
    |   |-- Inventory Framework
    |   |-- Narrative Quest And Dialogue
    |   |-- Quest Editor
    |   |-- Dialogue Plugin
    |   |-- Advanced Turn Based Tile Toolkit
    |   |-- Horror Engine
    |   |-- Survival Game Kit
    |   |-- RPG Engine Toolkit
    |   `-- Interaction / Inventory / Quest Packs
    |-- Animation / Locomotion / Characters
    |   |-- ALS Community
    |   |-- ALS Refactored
    |   |-- Motion Symphony
    |   |-- MoveIt Locomotion System
    |   |-- Procedural Animation Framework
    |   |-- Simple Procedural Walk
    |   |-- Dragon IK
    |   |-- Power IK
    |   |-- Kawaii Physics
    |   |-- Runtime Retargeting Plugins
    |   |-- Character Interaction Packs
    |   `-- Modular Animation Packs
    |-- AI / NPC / Agents
    |   |-- Mercuna 3D Navigation
    |   |-- Kythera AI
    |   |-- Don AI Navigation
    |   |-- AI Gameplay Path System
    |   |-- Behavior Tree Extensions
    |   |-- Utility AI Plugins
    |   |-- GOAP Plugins
    |   |-- Autonomix
    |   |-- LLM / ChatGPT Unreal Plugins
    |   `-- NPC Dialogue AI Plugins
    |-- Rendering / Shading / VFX
    |   |-- Extended Shading Pro
    |   |-- Ultra Volumetrics Packs
    |   |-- Fluid Ninja
    |   |-- PopcornFX
    |   |-- Niagara Extension Packs
    |   |-- Runtime Virtual Texture Tools
    |   |-- Better Decals
    |   |-- Weather FX Packs
    |   |-- Stylized Rendering Packs
    |   |-- Cel Shading Plugins
    |   `-- Custom Post Process Packs
    |-- Audio / Middleware
    |   |-- FMOD Studio
    |   |-- Audiokinetic Wwise
    |   |-- Master Audio
    |   |-- Runtime Audio Importer
    |   |-- Audio Analyzer Plugins
    |   `-- Dialogue Audio Tools
    |-- Networking / Multiplayer
    |   |-- Advanced Sessions Plugin
    |   |-- VR Expansion Plugin
    |   |-- Smooth Sync
    |   |-- SteamCore
    |   |-- EOSCore
    |   |-- Multiplayer With Blueprints
    |   |-- Dedicated Server Tools
    |   |-- Replication Graph Helpers
    |   `-- Voice Chat Plugins
    |-- UI / Productivity / Editor
    |   |-- Electronic Nodes
    |   |-- Blueprint Assist
    |   |-- Node Graph Assistant
    |   |-- Auto Size Comments
    |   |-- Asset Cleaner
    |   |-- Project Cleaner
    |   |-- Advanced Game Logging
    |   |-- Editor Scripting Helpers
    |   |-- Data Table Editor Tools
    |   |-- Localization Tools
    |   `-- In-Editor Debug Widgets
    |-- Import / Geospatial / Simulation
    |   |-- Cesium For Unreal
    |   |-- ArcGIS Maps SDK For Unreal
    |   |-- Microsoft AirSim Legacy
    |   |-- CARLA Unreal Integration
    |   |-- USD Pipeline Tools
    |   |-- OpenStreetMap Importers
    |   |-- GIS / 3D Tiles Plugins
    |   |-- LiDAR Point Cloud Tools
    |   `-- Photogrammetry Import Tools
    |-- XR / Virtual Production
    |   |-- Varjo OpenXR
    |   |-- MetaXR
    |   |-- Vive OpenXR
    |   |-- Virtual Camera Packs
    |   |-- Camera Calibration Packs
    |   |-- LED Wall Tools
    |   |-- DMX Fixture Packs
    |   `-- Mocap / Tracking Plugins
    `-- Web / Database / Services
        |-- VaRest
        |-- HttpGemini
        |-- WebSocket Plugins
        |-- JSON / REST Blueprint Plugins
        |-- SQLite Plugins
        |-- Firebase Plugins
        |-- PlayFab Plugins
        |-- Nakama Plugins
        |-- AWS / Azure / GCP Service Plugins
        `-- Discord / Twitch Integration Plugins
```

```text
Sources
|-- Epic official tools/editors
|   |-- https://dev.epicgames.com/documentation/unreal-engine/tools-and-editors-in-unreal-engine
|   |-- https://dev.epicgames.com/documentation/en-us/unreal-engine/level-editor-modes-in-unreal-engine
|   |-- https://dev.epicgames.com/documentation/en-us/unreal-engine/modeling-mode-in-unreal-engine
|   |-- https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-editors-in-unreal-engine
|   |-- https://dev.epicgames.com/documentation/en-us/unreal-engine/designing-visuals-rendering-and-graphics-with-unreal-engine
|   `-- https://dev.epicgames.com/documentation/en-us/unreal-engine/API/PluginIndex
|-- External plugin references
|   |-- https://voxelplugin.com/
|   |-- https://voxy.tools/
|   |-- https://dungeonarchitect.dev/
|   |-- https://docs.dungeonarchitect.dev/
|   |-- https://logicdriver.com/
|   |-- https://cesium.com/learn/unreal/
|   |-- https://www.ultradynamicsky.com/
|   |-- https://www.sidefx.com/products/houdini-engine/
|   `-- https://www.fab.com/
```
