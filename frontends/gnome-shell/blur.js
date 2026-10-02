// The picker's frosted backdrop (PICK-01): what lies under the panel,
// blurred and cut to the panel's rounded corners.
//
// Shell.BlurEffect in background mode reads the framebuffer it paints into,
// so it cannot be rounded by an offscreen effect. Instead the backdrop
// paints a clone of the window group (the wallpaper and the windows) under
// the panel, blurs that clone (actor mode) and masks its corners with a
// small shader, the outer effect.
import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';

const BLUR_RADIUS = 36;

const ROUND_DECLARATIONS = `
uniform vec2 size;
uniform float radius;
`;
// A signed distance to the rounded rectangle, one pixel of antialiasing.
const ROUND_CODE = `
vec2 p = cogl_tex_coord_in[0].xy * size;
vec2 q = abs(p - size * 0.5) - (size * 0.5 - vec2(radius));
float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
cogl_color_out *= clamp(0.5 - d, 0.0, 1.0);
`;

const RoundedCornersEffect = GObject.registerClass(
class WyeRoundedCornersEffect extends Shell.GLSLEffect {
    constructor(params) {
        super(params);
        this._sizeLocation = this.get_uniform_location('size');
        this._radiusLocation = this.get_uniform_location('radius');
    }

    vfunc_build_pipeline() {
        this.add_glsl_snippet(Cogl.SnippetHook.FRAGMENT, ROUND_DECLARATIONS, ROUND_CODE, false);
    }

    setGeometry(width, height, radius) {
        this.set_uniform_float(this._sizeLocation, 2, [width, height]);
        this.set_uniform_float(this._radiusLocation, 1, [radius]);
        this.queue_repaint();
    }
});

/**
 * Fills its parent (a BinLayout) without asking for room: the clone inside
 * is as big as the stage.
 */
export const Backdrop = GObject.registerClass(
class WyeBackdrop extends Clutter.Actor {
    constructor(radius) {
        super({
            clip_to_allocation: true,
            x_expand: true,
            y_expand: true,
            x_align: Clutter.ActorAlign.FILL,
            y_align: Clutter.ActorAlign.FILL,
        });
        this._radius = radius;
        this._clone = new Clutter.Clone({source: global.window_group});
        this.add_child(this._clone);
        this._round = new RoundedCornersEffect();
        // The first effect is the outer one: it rounds what the blur drew.
        this.add_effect_with_name('wye-round', this._round);
        this.add_effect_with_name('wye-blur', new Shell.BlurEffect({
            mode: Shell.BlurMode.ACTOR,
            radius: BLUR_RADIUS,
            brightness: 1.0,
        }));
        this.connect('notify::allocation', () => {
            const box = this.get_allocation_box();
            this._round.setGeometry(box.get_width(), box.get_height(), this._radius);
        });
    }

    vfunc_get_preferred_width(_forHeight) {
        return [0, 0];
    }

    vfunc_get_preferred_height(_forWidth) {
        return [0, 0];
    }

    /**
     * Lines the clone up with the stage: the backdrop's top-left corner is
     * at `x`, `y` on the stage.
     *
     * @param {number} x
     * @param {number} y
     */
    setStagePosition(x, y) {
        this._clone.set_position(-x, -y);
    }
});
