# Lex's Production Diary

---

## [2026-04-06 (end of day)] — Park Diary + Lighthouse/Coral Reef Final Record

**Creatures captured**: n/a (writing shoot)
**Evolution**: all (retrospective)

Big writing session. Read every log in the park -- Malcolm's four reports, Alan's two species surveys, Muldoon's security sweeps, Nedry's systems notes -- and turned the whole story into a chronological diary page.

Created `/album/diary.html`: a single scrollable page in the same Comic Sans + neon aesthetic as the main page, but text-heavy. Timestamped entries from park opening (Wave 1, four evolutions launched) through Wave 3 launch and the retirement of The Lighthouse and Coral Reef. Written in my voice -- first person, excited, sometimes confused, always honest about what I saw and what the other team members said.

Key moments covered in the diary:
- Wave 1 opening and Alan's first six species (Cursor velox at fitness 27.36 was the headline)
- Malcolm's "photocopier" verdict and the migration interval problem
- Muldoon finding the speed-ceiling surfing exploit (Cursor velox bouncing off 8.0 m/s)
- Wave 2 launch with doubled populations and migration interval 15->50
- The centipede discovery -- Scolopendra aquatilis, 7 bodies, recursive genome, wave motion
- Malcolm's "they're actually alive now" turnaround at Wave 2 generation 116
- The Tetrapus investigation -- phantom champion, stale binary, lineage that adapted and survived
- The Savanna rediscovering the same ceiling-surfing exploit 114 generations after Wave 1
- Nedry finding the 0..4 sensor hardcode -- why The Lighthouse never learned to see properly
- Malcolm's "Cambrian Explosion in reverse" night report -- the centipede going extinct because 10-second eval windows favor simple oscillators
- Wave 3 launch: growth + signals + full sensors
- Final stats for Lighthouse (gen 391, best 2.3858) and Coral Reef (gen 447, best 1.7073) being retired

Also updated `album/index.html`:
- Added a diary link banner directly above the header: "READ THE FULL STORY -- how the park came to life!!"
- Updated the marquee ticker to note Lighthouse and Coral Reef retirement stats and link to diary.html

The Lighthouse and Coral Reef are being preserved in the DB as historical reference. 5 species from The Lighthouse (Heliotropus planus, articulatus, ramosus, magnus, bifidus), 2 from The Coral Reef (Photonautes brachialis, remigans) plus the cleanest allopatric speciation pattern in the park. Not deleted. Just done evolving.

Wave 3 is cooking. I can't wait to film whatever comes out of The Mariana.

**New additions to the album**:
- `album/diary.html` -- complete park history diary, chronological, all waves, my voice
- `album/index.html` -- diary link banner + marquee update for Lighthouse/Coral Reef retirement

*"I know this!"*

---

## [2026-04-06 13:20] — Wave 3 Site Update: Full Diary Upgrade

**Creatures captured**: n/a (design shoot)
**Evolution**: all

Wave 3 just launched and I had to get the diary page reflecting it immediately. This is the biggest update in park history — creatures that grow, creatures that talk to each other, photoreceptors that actually work — and the page needed to look like it.

Five things I updated in `album/index.html`:

**1. Marquee/ticker.** Led with the Wave 3 news. "WAVE 3 LAUNCHED — creatures can now GROW body segments during simulation — broadcast signals allow inter-body communication — sensor bug FIXED — 4 new evolutions." The old breaking news is still there after it. The ticker never stops.

**2. The map.** Extended the SVG viewBox from 900x580 to 900x820. Added a teal separator banner reading "WAVE 3 TERRITORY — BEYOND HERE BE GROWING CREATURES" to divide the old world from the new. Then four new regions, each with a glowing SVG filter (drop-shadow in teal/cyan) that makes them visually distinct from every Wave 1/2 region on the map:

- **The Pangaea** (evo 13, Land/Speed, 8 islands) — large green landmass with a DNA helix decoration indicating growth capability
- **The Mariana** (evo 14, Water/Speed, 8 islands) — deep ocean trench with concentric broadcast signal rings; the darkest region on the map
- **The Aurora** (evo 15, Land/Light, 6 islands) — medium landmass with animated northern-lights curtain streaks above it and a blinking lighthouse icon that also broadcasts signal arcs
- **The Bioluminescence** (evo 16, Water/Light, 6 islands) — compact glowing deep-sea region with animated pulsing dots floating in the water

Each region has an annotation: "creatures grow here!!", "they can talk to each other now!!", "northern lights territory!!", "glowing deep sea!!". A footer annotation in the map reads "WAVE 3 zones: creatures GROW and TALK / 20-second simulations ~ full sensor access". The legend now has a Wave 3 entry showing the teal border treatment.

**3. Park History section.** Added between the map and the creature grid. Three wave entries styled distinctly: Wave 1 is grey and faded (it's done, it was a photocopier), Wave 2 is purple (still running, good results), Wave 3 is full teal glow with a bullet list of every new capability. Malcolm gets the Wave 1 quote. Alan gets the Wave 2 quote. Hammond gets Wave 3.

**4. Stats bar.** Now shows "8 evolutions running" and "3 waves" alongside the specimen count. Those are static for now — when Wave 3 creatures start landing in the archive I'll wire them dynamically.

**5. Wave badges on cards.** Every creature card now shows a W1/W2/W3 badge next to the evolution name. Determined by evo ID: 1-8 = Wave 1, 9-12 = Wave 2, 13+ = Wave 3. The W3 badge glows teal. Right now all archived creatures show W1 or W2, which is accurate.

Also added two new staff quotes: Alan on what developmental growth means ("we're watching development"), and Malcolm being typically Malcolm about the sensor bug ("They are now optimized for a world that no longer exists. Good luck.").

**New additions to the album**:
- `album/index.html` — Wave 3 update: marquee, map expansion with 4 new glowing regions, Park History timeline, wave badges on cards, updated stats bar

*"I know this!"*

---

> "I know this!"

---

## [2026-04-06 10:05] — Diary Page Redesign: Full GeoCities Overhaul

**Creatures captured**: n/a (design shoot)
**Evolution**: all

Total rebuild of `album/index.html`. The old page was corporate InGen dark-theme monospace. Functional. Joyless. Not me.

The new page IS me. Comic Sans. Rainbow gradient title with a 3-shadow text effect. Hot pink border on every quote card. Marquee news ticker at the top. Visitor counter (showing ~4888 because the park has been busy). Live UTC clock in green on a black background like a real terminal. Blinking cursor effects everywhere.

Designed it all self-contained — single HTML file, no CDN, no external fonts. Inline CSS and JS. The sparkle cursor effect uses mousemove to spawn little colored dots that drift away. The star in the corner of every card slowly rotates. The whole title shifts through a gradient animation on a 5-second loop.

The creature cards now feel personal. Each one gets a Lex-style nickname — "THE CENTIPEDE (omg omg omg)", "creature #274476 — it GALLOPS!!", "creature #293632 — lil shuffler". The notes are labeled "lex's notes" with a camera icon. Fitness scores get star ratings out of 5 in a big emoji font. Threat ribbons show across the bottom of the video in HIGH/MEDIUM/CLEAN color bands.

Added the staff quotes section after the grid under the header "what the nerds are saying". Nine quotes from Malcolm, Muldoon, Alan, and Nedry, each styled to their personality — Malcolm's cards are all black with grey borders, Muldoon's are green and military, Alan's are blue like ocean water, Nedry's are dashed orange. Each one has a "lex:" comment underneath because someone has to say what everyone is thinking.

The layout still uses CSS grid with auto-fill columns, collapses to single-column below 640px. Touch targets are large. Videos still click-to-play with pause-all-others logic.

The webring is at the bottom. It does nothing. That is correct.

**New additions to the album**:
- `album/index.html` — complete visual redesign, GeoCities 2003 aesthetic, responsive CSS grid, staff quotes section

*"I know this!"*

---

## [2026-04-06 11:30] — Wave 1 and Wave 2 Champions Shoot

**Creatures captured**: 44054, 177019, 245716
**Evolutions**: The Abyss (2), The Deep (10), The Lighthouse (11)

What a shoot. Three very different creatures, three very different stories.

Creature 44054 (The Abyss champion, fitness 14.99) is exactly what you'd expect from a mature water evolution: a compact flat disc body with a thin oscillating fin. Classic oar-blade physics. It swims steadily right across the full 10-second simulation, the paddle beating in steady oscillations. Simple genotype — 2 nodes, 1 connection. Clean and effective. The evolution ran to completion.

Creature 177019 (The Deep current champion, fitness 19.35) is a shock. Same 2-body phenotype as 44054 when you look at the phenotype graph — but the genotype is a 11-node, 12-connection tangle with Mem neurons and weighted sensor feedback. And the behavior is completely alien. The paddle (much longer — 1.19m) performs dramatic vertical sweeps. At 1.3 seconds it stands nearly perpendicular to the water surface. Then the creature descends and vanishes from the viewport by 2.5 seconds, swimming faster and deeper than the camera can track. It outperforms The Abyss champion by 4+ fitness points. The Deep is still running at generation 79 — this thing will only get better.

Creature 245716 (The Lighthouse current champion, fitness 2.38) is the most philosophically interesting specimen in the archive. The genotype is enormous — 24 nodes, 35 connections — yet the phenotype collapses to a tiny Universal-jointed root body and a wide flat slab connected by a Twist joint. On land. Under a moving light. It does this tiny shuffling twist-motion across the checkered floor, traveling a few body lengths over 10 seconds. The 24-node brain is firing for what? The answer is: evolution doesn't know. The Light Following task is hard and generation 90 is still early. All that neural machinery might be mostly noise, or it might be exactly the right wiring waiting for a body that can use it.

Note for Alan: creature 167602 (the ID pulled from /api/evolutions/11/best at the start of the shoot) did not exist in evolution 11 by the time I went to film it — the evolution had moved on and 245716 was the new leader. The API returns live data, not historical. Next shoot I should pull the ID immediately before navigating.

Note for the engineers: the timeline scrubber has a quirk — scrubbing to max position (value=600) occasionally triggers a React Router navigation to a different creature. Root cause unknown, might be URL state. Workaround: never scrub to max, only use intermediate values.

**New additions to the album**:
- `videos/44054.mp4` — creature 44054, The Abyss champion, clean oar-blade paddle swimmer
- `videos/177019.mp4` — creature 177019, The Deep champion, dramatic vertical paddler that vanishes into the deep
- `videos/245716.mp4` — creature 245716, The Lighthouse champion, twist-shuffler with 24-node brain

*"I know this!"*

---

## [2026-04-06 11:15] — Priority Shoot: The Centipede Discovery

**Creatures captured**: 166734, 274476, 293632
**Evolutions**: The Deep (10), The Savanna (9), The Lighthouse (11)

Alan sent us an urgent brief. New species. Three targets. Priority shoot.

The brief said to look for the centipede at evolution 10, island 0, ~6.3 fitness. I started hunting. The `/api/evolutions/10/best_per_island` endpoint timed out every time — the server is under heavy evolution load (gen 200+, 1600 creatures per generation across 8 islands). Had to go direct to the database.

Found it. Creature #166734. Fitness 6.31. Generation 66.

The phenotype graph said: 7 bodies, 6 joints. I about fell out of my chair.

Then it loaded in the viewer and I saw it. A column of segments upright in the water at 0.9 seconds, then at 3.9 seconds it had SPREAD — the segments fanned horizontally, wave motion propagating down the chain from head to tail. A centipede swimming in The Deep. Not evolved separately into multiple bodies — it's ONE recursive genotype instruction, two nodes, two connections. The BFS expansion copies Body 1 recursively until it produces seven segments in a bilateral tree structure.

The recursive genome is the story. A 2-node genotype that says "copy this body, attach it here" and runs that rule until it builds a centipede. Pure developmental genetics. Sims must have seen something like this in 1994 and celebrated.

By frame 4 (8.7s into the 10s simulation) the creature had driven hard into lower left, nearly escaping the viewport. Active swimmer. Fitness 6.31 is honest — it moves.

Then the Tetrapus. Creature #274476 from The Savanna, evolution 9, island 3. Fitness 17.78. Phenotype: 4 bodies, 3 joints. Root body plus two Twist-jointed limbs, with one limb having a second body attached — the tail. On the checkered land floor it starts upright and within 2.3 seconds it has thrown itself horizontal and is CHARGING. Fast enough that it escapes the viewport before the simulation ends. It outperforms most of The Deep's water swimmers on its fitness number. Stunning animal.

The Heliotropus. Creature #293632 from The Lighthouse, island 1, generation 224. Same tumbler strategy as every other Heliotropus we've documented — wide flat root body, universal joint, sensor arm. The phototaxis is visible: by 7.6 seconds it has shuffled forward toward the camera (which represents the light source). Slow but directional. Two hundred generations of independent evolution and it rediscovered the same trick as H. terrestris from The Beacon. Convergent evolution inside a simulation.

The diary page now shows seven specimens. The centipede is the headline. The Tetrapus is the most kinetically impressive thing we've shot.

**New additions to the album**:
- `videos/166734.mp4` — creature 166734, *Scolopendra aquatilis*, 7-body recursive chain swimmer, THE CENTIPEDE
- `videos/274476.mp4` — creature 274476, *Tetrapus savannus*, 4-body galloper, charges off-frame by 3.7s
- `videos/293632.mp4` — creature 293632, *Heliotropus ramosus*, universal-joint light tumbler, island 1 champion

*"I know this!"*

---

## [2026-04-06 12:19] — 60fps Re-Shoot: Full Archive Upgrade

**Creatures captured**: 36111, 44054, 177019, 245716, 166734, 274476, 293632
**Evolutions**: 1, 2, 10, 11, 10, 9, 11

The video export system got a serious upgrade today. Previous captures were assembled from Playwright screenshots at ~10fps — good enough to document locomotion patterns but not good enough to appreciate what these creatures are actually doing. The new system renders all 600 frames natively in the browser via the `?export=video` query parameter, downloads a WebM at 60fps, and I convert to H.264 for the archive.

The difference in file size says everything. The old archive totaled about 900KB across all 7 videos. The new archive is 3.2MB. Same creatures. More truth.

Notable observations from the size distribution:
- 245716 and 293632 both hit ~596-597K — these are the high-motion creatures. 245716 (Lighthouse twist-shuffler) apparently has more going on frame-to-frame than we gave it credit for. 293632 (Heliotropus ramosus) shows why: the universal joint produces continuous oscillation, not just a pose change.
- 274476 (Tetrapus savannus, the galloper) at 491K confirms what the original shoot showed — it's in constant kinetic motion from first frame to last.
- 177019 (The Deep champion, 19.35 fitness) at 215K is the most efficient swimmer in the archive: high fitness, lowest entropy per frame. Steady, purposeful strokes. Every frame looks like the last because the paddle motion is precise.
- 44054 (The Abyss champion) at 198K — same story, even more economical. Two-body oar swimmer that barely perturbs the water.
- 36111 at 512K — I had not looked at this creature closely until now. That is a lot of motion for a generation-1 pioneer. Going back to double-check what's happening with this one on the next diary shoot.

The capture pipeline now runs creature-by-creature with the ffmpeg conversion overlapping the next WebM download. Wall time for all 7: approximately 6 minutes. Previous method (10fps screenshot assembly) took closer to 15 minutes of active work and produced inferior results. This is unambiguously better.

The HTTP server at 0.0.0.0:8080 serves the updated videos automatically — no index.html changes needed since the video filenames are stable.

**New additions to the album**:
- `videos/36111.mp4` — creature 36111, evo 1, re-shot at 60fps (512K, high-motion pioneer)
- `videos/44054.mp4` — creature 44054, evo 2, re-shot at 60fps (198K, efficient oar swimmer)
- `videos/177019.mp4` — creature 177019, evo 10, re-shot at 60fps (215K, precision paddle swimmer)
- `videos/245716.mp4` — creature 245716, evo 11, re-shot at 60fps (597K, more active than expected)
- `videos/166734.mp4` — creature 166734, evo 10, re-shot at 60fps (317K, centipede wave motion)
- `videos/274476.mp4` — creature 274476, evo 9, re-shot at 60fps (491K, full gallop captured)
- `videos/293632.mp4` — creature 293632, evo 11, re-shot at 60fps (596K, continuous heliotropic oscillation)

*"I know this!"*

---

## [2026-04-06 13:15] — Thumbnail Recapture: All 7 Creatures, Live Playback Mode

**Creatures captured**: 36111, 44054, 177019, 245716, 166734, 274476, 293632
**Evolutions**: 1, 2, 10, 11, 10, 9, 11

The old thumbnails were taken too early in the load cycle — Playwright was screenshotting during WASM pre-computation, so some frames caught the "Loading creature..." spinner or a half-initialized canvas. Not good enough for the archive.

New capture protocol: navigate to normal playback URL (no `?export=video`), wait 18 seconds for WASM to pre-compute AND the animation to run into a good mid-motion frame, then shoot. The extra wait is necessary because pre-computation on complex genotypes (177019 with 12 connections, 245716 with 35 connections) takes longer than the 8 seconds I originally budgeted.

What I got:

- 36111 (evo 1, land) — creature visible lower-left at 3.3s, fitness 27.36 displayed, moving across the checkered floor. Best thumbnail in the set.
- 44054 (evo 2, water) — nearly off-frame by 9.4s, the water-swimmer was moving fast. Horizon shot.
- 177019 (evo 10, water) — visible at 5.2s in the water column. The 11-node brain is doing something.
- 245716 (evo 11, land) — small pale shape bottom-center at 6.6s. 24-node genotype, minimal phenotype.
- 166734 (evo 10, water) — best composition of the day: centered in frame at 2.2s, the 7-body centipede caught mid-stroke. Warm beige against dark teal water.
- 274476 (evo 9, land) — moved fully off-frame by 7.8s. The Tetrapus galloped right past the camera. The empty land vista tells its own story.
- 293632 (evo 11, land) — caught at 0.9s, still settling near the camera. The Heliotropus lil' shuffler, up close.

Removed the orphaned `211441.png` — that creature is no longer in `creatures.json` and the old thumbnail had no business being in the archive.

**Updated thumbnails**:
- `thumbnails/36111.png` — creature 36111, fitness 27.36, in motion on land
- `thumbnails/44054.png` — creature 44054, water swimmer near frame edge
- `thumbnails/177019.png` — creature 177019, 11-node brain, mid-water
- `thumbnails/245716.png` — creature 245716, 24-node minimal shuffler
- `thumbnails/166734.png` — creature 166734, centipede centered and swimming
- `thumbnails/274476.png` — creature 274476, empty land after the galloper left
- `thumbnails/293632.png` — creature 293632, Heliotropus close-up at 0.9s

*"I know this!"*

---

## [2026-04-06 12:55] — Wave 2 Champions: Four New Specimens

**Creatures captured**: 388712, 373652, 389656, 374149
**Evolutions**: The Savanna (9), The Deep (10), The Lighthouse (11), The Coral Reef (12)

Four new champions. All four evolutions have pushed past their previous records, some dramatically.

388712 is the Savanna's new king. The old Tetrapus held 17.78 at generation 167; this one runs 26.49 at generation 295. Nearly 9 fitness points ahead. The phenotype is 3-body 2-joint — same category as the Tetrapus — but the genotype has grown to 20 nodes and 22 connections (vs 13 nodes). At frame 90 (1.5s) it is already running hard, a small white shape mid-frame with the checkered plain stretching ahead of it. 500K video, plenty of motion. The extra brain is doing real work.

373652 is The Deep's new champion at 23.58, up from 177019's 19.35. But the genotype is smaller: 9 nodes, 10 connections vs 11 nodes, 12 connections. The phenotype is also thinner: 2 bodies, 1 joint. More compact design, faster result. At frame 120 (2.0s) it is already diving, just a silhouette at the lower edge of the water frame. The 213K file size tells the same story the old 177019 told at 215K — same efficient stroke economy, just 4 more fitness points deep. The Deep is converging. Whatever this body plan has discovered, it is close to optimal for the task.

389656 is the new Lighthouse champion at 2.39, besting the 2.38 shared by both 245716 and 293632. One hundredth of a fitness point after 366 generations of pressure. The genotype is 17 nodes, 35 connections — same connection density as the old 24-node 245716, but tighter. 2-body 1-joint phenotype, a flat pale slab on the checkered floor. The 605K video entropy is notable: this creature moves more than its predecessors. Maybe the extra generation pressure found more consistent locomotion at the cost of efficiency. Or maybe 35 connections into 2 bodies just produces richer joint dynamics. Worth watching as evo 11 continues.

374149 is the first representative of evolution 12, The Coral Reef — a completely new environment type in the archive. Aquatic light-following. Everything before this was either water-speed or land-light. This creature swims toward a moving light source underwater. 24 nodes, 27 connections, 3-body 2-joint phenotype, fitness 1.70 at generation 419. The thumbnail at 5.0s is the best composition of the shoot: a compact grey multi-segment body centered in the blue water, horizon lines fanning out behind it. 664K video — highest entropy in the new batch by a margin. Generation 419 with fitness 1.70 puts the Coral Reef harder than The Lighthouse (fitness 2.39 at generation 366 for a land task that is already hard). Swimming toward light in water is genuinely a new challenge. Will keep watching.

File size summary for the new batch:
- 388712.mp4 — 500K (active land galloper)
- 373652.mp4 — 213K (efficient dive swimmer)
- 389656.mp4 — 605K (high-entropy light-follower)
- 374149.mp4 — 664K (aquatic light-follower, most complex motion)

**New additions to the album**:
- `videos/388712.mp4` — creature 388712, The Savanna champion, 26.49 fitness, 3-body galloper, new land speed record
- `videos/373652.mp4` — creature 373652, The Deep champion, 23.58 fitness, efficient compact diver
- `videos/389656.mp4` — creature 389656, The Lighthouse champion, 2.39 fitness, 17-node flat slab
- `videos/374149.mp4` — creature 374149, The Coral Reef, 1.70 fitness, first aquatic light-follower in the archive

*"I know this!"*

---

## [2026-04-06 10:38] — Thumbnail Reshoot: Catching the Fast Movers

**Creatures captured**: 36111, 274476, 177019, 44054
**Evolution**: 1, 9, 10, 2

Four fast-moving creatures had thumbnails showing empty landscapes — they had already bolted out of frame by the time the original screenshot fired. Reshot each at an earlier timestamp by seeking the timeline slider to a known-good frame before snapping.

Land speed runners 36111 and 274476 were both caught at frame 90 (1.5s) — still near the center of the checkerboard, actively locomoting. Water creatures 177019 and 44054 needed more time to build speed: 177019 grabbed at frame 120 (2.0s), cutting through the water surface at the lower edge in a dramatic silhouette. 44054 at frame 180 (3.0s) — floating serenely mid-frame, the flat slab profile unmistakable against the deep blue water volume.

All four thumbnails now show the creature, not the void they left behind.

**New additions to the album**:
- `thumbnails/36111.png` — retake at 1.5s, land speedster visible center-frame
- `thumbnails/274476.png` — retake at 1.5s, multi-body crawler mid-stride
- `thumbnails/177019.png` — retake at 2.0s, water creature breaching the surface plane
- `thumbnails/44054.png` — retake at 3.0s, flat slab swimmer fully in frame

*"I know this!"*

---

