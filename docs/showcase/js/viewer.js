'use strict';
// three.js side of the showcase: models from glb.js, items from items.js, actors, stages.
//
// Frames. A glb from the build is in the glTF frame, which is the game's draw frame of a mob:
// y up, the mob looks along +z, +x is its left, feet at the origin, blocks. The game model frame
// is that frame times scale(1, -1, -1), 1.501 blocks up. An item in a hand socket therefore hangs
// under the socket node with  scale(1, -1, -1) * translate(t/16) * Rx * Ry * Rz * scale *
// translate(-0.5)  from display.thirdperson_<hand>hand of the item model: the same chain the
// game runs after the socket's rotX(-90) rotY(180), which the glb already holds.
//
// Items. An element model is built from its JSON elements. A sprite model (parent item/generated
// or item/handheld, such as the swords) is extruded the way the game does it: the sprite as a
// front and a back quad, transparent texels cut, a side face on every texel edge next to a
// transparent texel; its display block comes from the parent (embed.py resolves it).
(function () {
  const T = THREE;
  const rad = (d) => (d * Math.PI) / 180;

  // ------------------------------------------------------------------ models
  const parsed = {};
  SC.loadModel = (id) => {
    if (parsed[id]) return parsed[id];
    if (!SC.GLB[id]) return Promise.reject(new Error('no model ' + id));
    const bytes = Uint8Array.from(atob(SC.GLB[id]), (c) => c.charCodeAt(0));
    return (parsed[id] = new Promise((done, fail) =>
      new T.GLTFLoader().parse(bytes.buffer, '', (gltf) => {
        gltf.scene.traverse((o) => {
          if (!o.isMesh) return;
          for (const key of ['map', 'emissiveMap']) {
            const tex = o.material[key];
            if (tex) nearest(tex);
          }
          // Glow parts (full-bright in the game) are lights: a scene's fog does not swallow them.
          if (o.material.emissiveMap) o.material.fog = false;
        });
        done(gltf);
      }, fail)
    ));
  };
  function nearest(tex) {
    tex.magFilter = tex.minFilter = T.NearestFilter;
    tex.generateMipmaps = false;
    tex.needsUpdate = true;
  }

  // ------------------------------------------------------------------ items
  // How the UV of an element face runs (vanilla FaceBakery): [u axis, sign], [v axis, sign].
  const EL_FACES = {
    north: [[0, -1], [1, -1], [0, 0, -1]], south: [[0, 1], [1, -1], [0, 0, 1]],
    west: [[2, 1], [1, -1], [-1, 0, 0]], east: [[2, -1], [1, -1], [1, 0, 0]],
    up: [[0, 1], [2, 1], [0, 1, 0]], down: [[0, 1], [2, -1], [0, -1, 0]],
  };
  function rotateEl(p, rot) {
    if (!rot) return p;
    const a = rad(rot.angle), i = 'xyz'.indexOf(rot.axis), j = (i + 1) % 3, k = (i + 2) % 3;
    const q = p.map((c, n) => c - (rot.origin ? rot.origin[n] : 0));
    const r = q.slice();
    r[j] = q[j] * Math.cos(a) - q[k] * Math.sin(a);
    r[k] = q[j] * Math.sin(a) + q[k] * Math.cos(a);
    return r.map((c, n) => c + (rot.origin ? rot.origin[n] : 0));
  }
  // Item textures are decoded once before any actor exists, so the first frame already has them.
  // A sprite also keeps the alpha of its texels: the extrusion needs to know which are empty.
  const itemTex = {}, itemAlpha = {};
  let itemsReady = null;
  SC.itemsReady = () =>
    itemsReady ||
    (itemsReady = Promise.all(Object.entries(SC.ITEMS).map(([id, item]) => new Promise((done) => {
      const img = new Image();
      img.onload = img.onerror = () => {
        const tex = (itemTex[id] = new T.Texture(img));
        tex.encoding = T.sRGBEncoding;
        nearest(tex);
        if (!item.model.elements && img.width) itemAlpha[id] = alphaOf(img);
        done();
      };
      img.src = item.texture;
    }))));
  function alphaOf(img) {
    const c = document.createElement('canvas');
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const rgba = g.getImageData(0, 0, img.width, img.height).data;
    const a = new Uint8Array(img.width * img.height);
    for (let i = 0; i < a.length; i++) a[i] = rgba[i * 4 + 3];
    return { w: img.width, h: img.height, a };
  }
  // Two triangles of a quad, counter-clockwise seen from outside (n is the outward normal).
  function addQuad(pos, uv, corners, uvs, n) {
    const a = new T.Vector3(...corners[0]), b = new T.Vector3(...corners[1]), c = new T.Vector3(...corners[2]);
    const order = b.sub(a).cross(c.sub(a)).dot(new T.Vector3(...n)) < 0 ? [0, 2, 1, 0, 3, 2] : [0, 1, 2, 0, 2, 3];
    for (const i of order) {
      pos.push(...corners[i]);
      uv.push(...uvs[i]);
    }
  }
  function itemGeometry(pos, uv) {
    const geo = new T.BufferGeometry();
    geo.setAttribute('position', new T.Float32BufferAttribute(pos, 3));
    geo.setAttribute('uv', new T.Float32BufferAttribute(uv, 2));
    geo.computeVertexNormals();
    return geo;
  }
  // The vanilla extrusion of a sprite (ItemModelGenerator) in item space, blocks: front at z 8.5 px,
  // back at 7.5 px over the whole sprite (alphaTest drops the empty texels, as the game's cutout
  // does), and per opaque texel a side face on each edge with an empty texel or the border
  // beyond it, in that texel's colour. Texel rows run down, item y runs up.
  function spriteMesh(id) {
    const s = itemAlpha[id];
    if (!s) throw new Error('no pixels for sprite ' + id);
    const { w, h, a } = s;
    const z0 = 7.5 / 16, z1 = 8.5 / 16;
    const opaque = (x, y) => x >= 0 && y >= 0 && x < w && y < h && a[y * w + x] > 0;
    const pos = [], uv = [];
    const whole = [[0, 0], [1, 0], [1, 1], [0, 1]];
    addQuad(pos, uv, whole.map(([x, y]) => [x, y, z1]), whole, [0, 0, 1]);
    addQuad(pos, uv, whole.map(([x, y]) => [x, y, z0]), whole, [0, 0, -1]);
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        if (!opaque(x, y)) continue;
        const x0 = x / w, x1 = (x + 1) / w, y0 = (h - 1 - y) / h, y1 = (h - y) / h;
        const mid = [(x + 0.5) / w, 1 - (y + 0.5) / h];
        const c4 = [mid, mid, mid, mid];
        if (!opaque(x, y - 1)) addQuad(pos, uv, [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]], c4, [0, 1, 0]);
        if (!opaque(x, y + 1)) addQuad(pos, uv, [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], c4, [0, -1, 0]);
        if (!opaque(x - 1, y)) addQuad(pos, uv, [[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]], c4, [-1, 0, 0]);
        if (!opaque(x + 1, y)) addQuad(pos, uv, [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], c4, [1, 0, 0]);
      }
    return new T.Mesh(itemGeometry(pos, uv), new T.MeshLambertMaterial({ map: itemTex[id], alphaTest: 0.5 }));
  }
  // The item model as a mesh in item space (blocks, 0..1): elements from its JSON, or a sprite.
  SC.itemMesh = (id) => {
    const item = SC.ITEMS[id];
    if (!item) throw new Error('no item ' + id);
    if (!item.model.elements) return spriteMesh(id);
    const pos = [], uv = [];
    for (const el of item.model.elements) {
      for (const [face, f] of Object.entries(el.faces)) {
        const [[ua, us], [va, vs]] = EL_FACES[face];
        const axis = EL_FACES[face][2].findIndex((c) => c !== 0);
        const hi = EL_FACES[face][2][axis] > 0;
        const [u0, v0, u1, v1] = f.uv;
        const corner = (fu, fv) => {
          const p = [0, 0, 0];
          p[axis] = hi ? el.to[axis] : el.from[axis];
          p[ua] = el.from[ua] + (el.to[ua] - el.from[ua]) * (us > 0 ? fu : 1 - fu);
          p[va] = el.from[va] + (el.to[va] - el.from[va]) * (vs > 0 ? fv : 1 - fv);
          return { p: rotateEl(p, el.rotation).map((c) => c / 16), uv: [(u0 + (u1 - u0) * fu) / 16, 1 - (v0 + (v1 - v0) * fv) / 16] };
        };
        const quad = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
        const n = rotateEl(EL_FACES[face][2], el.rotation ? { ...el.rotation, origin: [0, 0, 0] } : null);
        addQuad(pos, uv, quad.map((q) => q.p), quad.map((q) => q.uv), n);
      }
    }
    return new T.Mesh(itemGeometry(pos, uv), new T.MeshLambertMaterial({ map: itemTex[id] }));
  };
  // Socket frame -> item space, see the header.
  SC.heldMatrix = (display, hand) => {
    const d = display['thirdperson_' + hand + 'hand'];
    let [tx, ty, tz] = d.translation;
    let [rx, ry, rz] = d.rotation;
    if (hand === 'left') [tx, ry, rz] = [-tx, -ry, -rz];
    const M = () => new T.Matrix4();
    return M().makeScale(1, -1, -1)
      .multiply(M().makeTranslation(tx / 16, ty / 16, tz / 16))
      .multiply(M().makeRotationX(rad(rx))).multiply(M().makeRotationY(rad(ry))).multiply(M().makeRotationZ(rad(rz)))
      .multiply(M().makeScale(...d.scale))
      .multiply(M().makeTranslation(-0.5, -0.5, -0.5));
  };

  // ------------------------------------------------------------------ blocks
  // Blocks come from blocks_<set>.glb (build_blocks.py): one node per block model, named by its
  // model id, in the block frame (the glTF frame is the Minecraft block frame: +x east, y up,
  // +z south, origin at the block's north-west bottom corner, blocks). A placed block is the
  // node under a group at the cell: the blockstate's y turns it about the block centre (y 90 =
  // north to east, clockwise from above), an on_slab model sits slab_drop px lower.
  const blockGltf = {};
  let blocksReady = null;
  SC.blocksReady = () => {
    if (blocksReady) return blocksReady;
    if (!SC.BLOCKS) return Promise.reject(new Error('no blocks.js: rebuild the models'));
    const glbs = [...new Set(Object.values(SC.BLOCKS.models).map((m) => m.glb))];
    return (blocksReady = Promise.all([SC.itemsReady(), ...glbs.map((g) => SC.loadModel(g).then((gl) => (blockGltf[g] = gl)))]));
  };
  // A copy of one block model (after SC.blocksReady): origin at the block's north-west bottom corner.
  SC.blockModel = (model) => {
    const info = SC.BLOCKS.models[model];
    if (!info) throw new Error('no block model ' + model);
    let node = null;
    blockGltf[info.glb].scene.traverse((o) => {
      if (!node && (o.userData.name === model || o.name === model)) node = o;
    });
    if (!node) throw new Error(info.glb + ' has no node ' + model);
    const copy = node.clone(true);
    copy.position.set(0, 0, 0);
    copy.traverse((o) => {
      if (o.isMesh) o.castShadow = o.receiveShadow = true;
    });
    return copy;
  };
  // A placed block: {model, at: [x, y, z], rot, drop} as blocks.js structures list them.
  SC.placeBlock = (p) => {
    const g = new T.Group();
    g.position.set(p.at[0], p.at[1], p.at[2]);
    g.rotation.y = -rad(p.rot || 0);
    const m = SC.blockModel(p.model);
    m.position.set(-0.5, -(p.drop || 0) / 16, -0.5);
    g.add(m);
    return g;
  };
  // Every segment of a block's piece with its default states (bottom segment at y 0), turned.
  SC.blockPiece = (blockId, state, rot) => {
    const b = SC.BLOCKS.blocks.find((x) => x.id === blockId);
    if (!b) throw new Error('no block ' + blockId);
    const g = new T.Group();
    (b.part || [null]).forEach((part, i) => {
      let model = b.id + (part ? '_' + part : '');
      for (const [k, v] of Object.entries(state || {})) if (v === true) model += '_' + k;
      g.add(SC.placeBlock({ model, at: [0, i, 0], rot: rot || 0 }));
    });
    return g;
  };
  // Box of a model in blocks, inside its cell: [[x0, y0, z0], [x1, y1, z1]] from blocks.js bounds.
  SC.blockBox = (model, drop) => {
    const [lo, hi] = SC.BLOCKS.models[model].bounds;
    const d = (drop || 0) / 16;
    return [[lo[0] / 16 - 0.5, lo[1] / 16 - d, lo[2] / 16 - 0.5], [hi[0] / 16 - 0.5, hi[1] / 16 - d, hi[2] / 16 - 0.5]];
  };
  // A line box (the ghost outline the game draws with native_world_primitives), a hair larger
  // than the box so it never lies in a block face.
  SC.lineBox = (lo, hi, color) => {
    const pad = 0.004;
    const geo = new T.EdgesGeometry(new T.BoxGeometry(hi[0] - lo[0] + 2 * pad, hi[1] - lo[1] + 2 * pad, hi[2] - lo[2] + 2 * pad));
    const lines = new T.LineSegments(geo, new T.LineBasicMaterial({ color, fog: false }));
    lines.position.set((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2);
    return lines;
  };
  // The summon item over a pedestal standing in the cell at y 0 (data/altar.json pedestal_item):
  // the extruded sprite upright, centred height_px above the pedestal's bottom, scaled, turning
  // and bobbing with the tick: update(t).
  SC.pedestalItem = (itemId) => {
    const p = SC.ALTAR.pedestal_item;
    const g = new T.Group();
    const spin = new T.Group();
    const mesh = SC.itemMesh(itemId);
    mesh.position.set(-0.5, -0.5, -0.5);
    mesh.castShadow = true;
    spin.add(mesh);
    spin.scale.setScalar(p.scale);
    g.add(spin);
    g.update = (t) => {
      spin.rotation.y = rad(30 + t * p.spin_deg_per_tick);
      spin.position.y = (p.height_px + p.bob_px * Math.sin((t / p.bob_ticks) * 2 * Math.PI)) / 16;
    };
    g.update(0);
    return g;
  };

  // ------------------------------------------------------------------ actors
  const LOOPS = new Set(['idle', 'walk', 'block', 'circle_strafe']);

  // One instance of a model with its own mixer. hold: {socket: item id} adds items beyond the
  // ones the mob's rig.json puts in its hands. Parts with a state flag in rig.json show only while
  // setState() has the flag on ('!flag': only while it is off), as the game will draw them.
  class Actor {
    constructor(id, gltf, hold) {
      this.id = id;
      this.root = new T.Group();
      this.model = gltf.scene.clone(true);
      this.root.add(this.model);
      this.mixer = new T.AnimationMixer(this.model);
      this.clips = {};
      for (const c of gltf.animations) this.clips[c.name] = c;
      this.actions = {};
      const rig = SC.RIGS[id];
      const items = {};
      if (rig) for (const s of rig.sockets) if (s.item) items[s.name] = s.item.split(':')[1];
      Object.assign(items, hold || {});
      for (const [socket, item] of Object.entries(items)) this.hold(socket, item);
      this.model.traverse((o) => {
        if (o.isMesh) o.castShadow = o.receiveShadow = true;
      });
      this.states = Object.entries((rig && rig.states) || {}).map(([part, flag]) => {
        const node = this.node(part);
        if (!node) throw new Error(id + ' has no part ' + part + ' for state ' + flag);
        return { node, flag: flag.replace(/^!/, ''), on: !flag.startsWith('!') };
      });
      this.setState([]);
    }
    node(name) {
      let found = null;
      this.model.traverse((o) => {
        if (o.userData.name === this.id + ':' + name) found = o;
      });
      return found;
    }
    hold(socket, item) {
      const node = this.node(socket);
      if (!node) throw new Error(this.id + ' has no ' + socket);
      const mesh = SC.itemMesh(item);
      mesh.matrixAutoUpdate = false;
      mesh.matrix.copy(SC.heldMatrix(SC.ITEMS[item].display, socket.endsWith('_l') ? 'left' : 'right'));
      node.add(mesh);
    }
    // flags: the mob state flags that are on, e.g. ['rage'].
    setState(flags) {
      for (const s of this.states) s.node.visible = flags.includes(s.flag) === s.on;
    }
    ticks(clip) {
      const c = this.clips[clip];
      return c ? Math.round(c.duration * SC.TICK) : 0;
    }
    loops(clip) {
      const r = SC.RIGS[this.id];
      return r && r.clips[clip] ? r.clips[clip].loop : LOOPS.has(clip);
    }
    // Plays a blend of clips at given times: [[clip, tick, weight], ...]; the rest is off.
    pose(layers) {
      const used = new Set();
      for (const [clip, tick, weight] of layers) {
        if (!this.clips[clip]) continue;
        let a = this.actions[clip];
        if (!a) {
          a = this.actions[clip] = this.mixer.clipAction(this.clips[clip]);
          a.setLoop(T.LoopOnce);
          a.clampWhenFinished = true;
          a.play();
        }
        const n = this.ticks(clip);
        const t = this.loops(clip) ? ((tick % n) + n) % n : Math.max(0, Math.min(n, tick));
        a.enabled = true;
        a.paused = false;
        a.time = t / SC.TICK;
        a.setEffectiveWeight(weight);
        used.add(clip);
      }
      for (const [clip, a] of Object.entries(this.actions)) if (!used.has(clip)) a.setEffectiveWeight(0);
      this.mixer.update(0);
    }
    // World points that hold the visible model over a clip (the corners of every mesh's box,
    // every other tick), for framing a camera.
    bounds(clip) {
      const pts = [];
      const n = this.clips[clip] ? this.ticks(clip) : 0;
      const add = (o) => {
        if (!o.visible) return;
        if (o.isMesh) {
          if (!o.geometry.boundingBox) o.geometry.computeBoundingBox();
          const { min, max } = o.geometry.boundingBox;
          for (let i = 0; i < 8; i++)
            pts.push(new T.Vector3(i & 1 ? max.x : min.x, i & 2 ? max.y : min.y, i & 4 ? max.z : min.z).applyMatrix4(o.matrixWorld));
        }
        for (const c of o.children) add(c);
      };
      for (let t = 0; t <= n; t += 2) {
        this.pose(this.clips[clip] ? [[clip, t, 1]] : []);
        this.root.updateMatrixWorld(true);
        add(this.root);
      }
      return pts;
    }
  }
  SC.actor = (id, hold) => Promise.all([SC.loadModel(id), SC.itemsReady()]).then(([g]) => new Actor(id, g, hold));

  // ------------------------------------------------------------------ stage
  // A canvas with its own renderer, camera, lights, ground and orbit controls.
  //
  // The lights are the ones preview.py renders with in Blender (EEVEE, view transform Standard):
  // a grey world of linear (0.28, 0.30, 0.35) and a sun of strength 2.4. Three r147 lights in
  // legacy mode reflect albedo x intensity x cos, Blender's sun albedo x strength / pi x cos, so the
  // sun is 2.4 / pi here and the world is an ambient light of its colour. The sun comes from the
  // same side: rotation (0.75, 0, -0.5) in Blender is a light from the mob's front right, high.
  const WORLD = new T.Color(0.28, 0.3, 0.35);
  const SUN = 2.4 / Math.PI;
  const SUN_DIR = new T.Vector3(-0.327, 0.732, 0.598).normalize();
  // The background is the same world grey, as the previews show it: linear (0.28, 0.3, 0.35) in sRGB.
  const CLEAR = new T.Color(0.28, 0.3, 0.35).convertLinearToSRGB().getHex();
  // The view of the model window and the cards: preview.py's three-quarter camera direction.
  const VIEW_DIR = new T.Vector3(5.5, 1.95, 5.5).normalize();

  function groundTexture() {
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    const g = c.getContext('2d');
    let s = 7;
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 16; x++) {
        s = (s * 1103515245 + 12345) >>> 0;
        const v = 52 + ((s >>> 16) % 14);
        g.fillStyle = `rgb(${v - 6},${v},${v - 10})`;
        g.fillRect(x, y, 1, 1);
      }
    g.fillStyle = 'rgba(0,0,0,0.18)';
    g.fillRect(0, 0, 16, 1);
    g.fillRect(0, 0, 1, 16);
    const tex = new T.CanvasTexture(c);
    tex.wrapS = tex.wrapT = T.RepeatWrapping;
    tex.encoding = T.sRGBEncoding;
    nearest(tex);
    return tex;
  }

  class Stage {
    constructor(canvas, opts) {
      opts = opts || {};
      this.canvas = canvas;
      this.renderer = new T.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: !!opts.snapshot });
      this.renderer.outputEncoding = T.sRGBEncoding;
      this.renderer.setClearColor(CLEAR, 1);
      this.renderer.shadowMap.enabled = true;
      this.renderer.shadowMap.type = T.PCFShadowMap;
      this.scene = new T.Scene();
      this.ambient = new T.AmbientLight(WORLD.clone(), 1);
      this.scene.add(this.ambient);
      const size = opts.ground || 9;
      this.sun = new T.DirectionalLight(0xffffff, SUN);
      this.sun.castShadow = true;
      const sc = this.sun.shadow;
      sc.mapSize.set(2048, 2048);
      sc.camera.left = sc.camera.bottom = -(size / 2 + 3);
      sc.camera.right = sc.camera.top = size / 2 + 3;
      sc.camera.near = 1;
      sc.camera.far = 80;
      sc.bias = -0.0005;
      sc.normalBias = 0.03;
      sc.radius = 2;
      this.sun.position.copy(SUN_DIR).multiplyScalar(30);
      this.scene.add(this.sun, this.sun.target);
      const tex = groundTexture();
      tex.repeat.set(size, size);
      this.ground = new T.Mesh(new T.PlaneGeometry(size, size), new T.MeshLambertMaterial({ map: tex }));
      this.ground.rotation.x = -Math.PI / 2;
      this.ground.position.y = -0.001;
      this.ground.receiveShadow = true;
      if (opts.ground !== 0) this.scene.add(this.ground);
      this.camera = new T.PerspectiveCamera(opts.fov || 32, 16 / 10, 0.1, 100);
      this.camera.position.set(...(opts.eye || [3.3, 2.1, 4.4]));
      const target = new T.Vector3(...(opts.target || [0, 1, 0]));
      if (opts.controls !== false) {
        this.controls = new T.OrbitControls(this.camera, canvas);
        this.controls.target.copy(target);
        this.controls.enableDamping = false;
        this.controls.update();
      } else {
        this.camera.lookAt(target);
      }
      this.actors = [];
      this.lamp = null;
    }
    add(actor) {
      this.scene.add(actor.root);
      this.actors.push(actor);
      return actor;
    }
    // Points the camera along the three-quarter view at world points (Actor.bounds), as close
    // as they allow: the picture is centred on their projected extent, and a point o (from that
    // centre) fits when |o.right| <= (d - o.dir) tan(h/2), the same for up and v/2.
    frame(pts, margin) {
      const right = new T.Vector3().crossVectors(new T.Vector3(0, 1, 0), VIEW_DIR).normalize();
      const up = new T.Vector3().crossVectors(VIEW_DIR, right).normalize();
      const range = (axis) => {
        let lo = Infinity, hi = -Infinity;
        for (const p of pts) {
          const v = p.dot(axis);
          lo = Math.min(lo, v);
          hi = Math.max(hi, v);
        }
        return (lo + hi) / 2;
      };
      const c = new T.Vector3().addScaledVector(right, range(right)).addScaledVector(up, range(up)).addScaledVector(VIEW_DIR, range(VIEW_DIR));
      const tv = Math.tan((this.camera.fov * Math.PI) / 360), th = tv * this.camera.aspect;
      const m = margin || 1.1;
      let d = 0;
      for (const p of pts) {
        const o = p.clone().sub(c);
        d = Math.max(d, (Math.abs(o.dot(right)) * m) / th + o.dot(VIEW_DIR), (Math.abs(o.dot(up)) * m) / tv + o.dot(VIEW_DIR));
      }
      this.camera.position.copy(c).addScaledVector(VIEW_DIR, d);
      this.camera.far = Math.max(100, d * 4);
      this.camera.updateProjectionMatrix();
      if (this.controls) {
        this.controls.target.copy(c);
        this.controls.update();
      } else this.camera.lookAt(c);
    }
    // The light and air of a scene, or null for the default stage. env: {background: '#rrggbb',
    // fog: density (FogExp2), fogColor, light: ambient factor, ambientColor, sun: sun factor,
    // sunColor, ground: '#rrggbb' tint of the ground, lamp: {at, color, intensity, distance}}; the
    // lamp is a point light on lampActor (a scene names it by env.lamp.actor). Glowing parts are
    // never fogged (loadModel), so a lure stays a light in the dark.
    setEnv(env, lampActor) {
      const e = env || {};
      this.renderer.setClearColor(e.background ? new T.Color(e.background) : CLEAR, 1);
      this.scene.fog = e.fog ? new T.FogExp2(new T.Color(e.fogColor || e.background || CLEAR), e.fog) : null;
      this.ambient.color.copy(e.ambientColor ? new T.Color(e.ambientColor) : WORLD);
      this.ambient.intensity = e.light != null ? e.light : 1;
      this.sun.color.set(e.sunColor || 0xffffff);
      this.sun.intensity = SUN * (e.sun != null ? e.sun : 1);
      this.ground.material.color.set(e.ground || 0xffffff);
      if (this.lamp) {
        this.lamp.parent.remove(this.lamp);
        this.lamp = null;
      }
      if (e.lamp && lampActor) {
        const l = e.lamp;
        this.lamp = new T.PointLight(l.color || 0xffd7a0, l.intensity != null ? l.intensity : 1.7, l.distance || 7, 1.4);
        this.lamp.position.set(...(l.at || [-0.35, 1.15, 0.3]));
        lampActor.root.add(this.lamp);
      }
    }
    render() {
      const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
      if (w && h && (w !== this.w || h !== this.h)) {
        this.w = w;
        this.h = h;
        this.renderer.setPixelRatio(window.devicePixelRatio || 1);
        this.renderer.setSize(w, h, false);
        this.camera.aspect = w / h;
        this.camera.updateProjectionMatrix();
        if (this.onResize) this.onResize();
      }
      this.renderer.render(this.scene, this.camera);
    }
    dispose() {
      if (this.controls) this.controls.dispose();
      this.renderer.dispose();
    }
  }
  SC.Stage = Stage;

  // The scale a mob is shown at in the model window and on its card: SC.mob(id, {view: {size}}).
  SC.viewSize = (id) => (SC.MOBS[id] && SC.MOBS[id].view && SC.MOBS[id].view.size) || 1;

  // One animation loop for the page; whoever is on screen registers a tick(dt seconds) callback.
  // A callback that throws is dropped and reports to its own onError, else to the page.
  const ticking = new Map();
  let last = 0;
  function frame(now) {
    const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
    last = now;
    for (const [f, onError] of ticking) {
      try {
        f(dt);
      } catch (e) {
        ticking.delete(f);
        (onError || SC.fail)(String(e && e.stack ? e.stack : e));
      }
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  SC.onFrame = (f, onError) => {
    ticking.set(f, onError);
    return () => ticking.delete(f);
  };
  SC.stopFrames = () => ticking.clear();

  // A still picture of a model in its first idle tick, for the cards of the index, framed by the
  // model's own bounds.
  let snapStage = null;
  SC.snapshot = (id) =>
    SC.actor(id).then((actor) => {
      if (!snapStage) {
        const c = document.createElement('canvas');
        c.width = 320;
        c.height = 320;
        c.style.width = c.style.height = '320px';
        snapStage = new Stage(c, { snapshot: true, controls: false, fov: 30, ground: 3 });
        snapStage.renderer.setSize(320, 320, false);
        snapStage.camera.aspect = 1;
        snapStage.camera.updateProjectionMatrix();
      }
      snapStage.add(actor);
      const s = SC.viewSize(id);
      actor.root.scale.setScalar(s);
      snapStage.frame(actor.bounds('idle'), 1.1);
      snapStage.renderer.render(snapStage.scene, snapStage.camera);
      const url = snapStage.canvas.toDataURL('image/png');
      snapStage.scene.remove(actor.root);
      snapStage.actors = [];
      return url;
    });
})();
