import videojs from 'video.js';
import type Player from 'video.js/dist/types/player';
import type MenuButton from 'video.js/dist/types/menu/menu-button';
import type MenuItem from 'video.js/dist/types/menu/menu-item';
import type Plugin from 'video.js/dist/types/plugin';
import type Component from 'video.js/dist/types/component';

// Proper casting for Video.js components
const MenuButtonClass = videojs.getComponent('MenuButton') as unknown as {
  new (player: Player, options?: unknown): MenuButton;
};
const MenuItemClass = videojs.getComponent('MenuItem') as unknown as {
  new (player: Player, options?: unknown): MenuItem;
};

export interface SourceOption {
  label: string;
  protocol: 'direct' | 'hls' | 'dash';
}

interface ProtocolMenuItemOptions {
  label: string;
  protocol: 'direct' | 'hls' | 'dash';
  onSelect: (protocol: 'direct' | 'hls' | 'dash') => void;
  selectedProtocol?: string;
  sources?: SourceOption[];
}

class ProtocolMenuItem extends MenuItemClass {
  private protocol: 'direct' | 'hls' | 'dash';
  private onSelect: (protocol: 'direct' | 'hls' | 'dash') => void;

  constructor(player: Player, options: ProtocolMenuItemOptions) {
    super(player, options);
    this.protocol = options.protocol;
    this.onSelect = options.onSelect;
    this.setAttribute('role', 'menuitemradio');
  }

  handleClick(event: Event) {
    super.handleClick(event);
    this.onSelect(this.protocol);
  }

  update() {
    const options = this.options_ as ProtocolMenuItemOptions;
    const selectedProtocol = options.selectedProtocol;
    this.selected(this.protocol === selectedProtocol);
  }
}

class SourceMenuButton extends MenuButtonClass {
  constructor(player: Player, options: ProtocolMenuItemOptions) {
    super(player, options);
    this.controlText('Source Protocol');
    this.addClass('vjs-source-selector');
    
    player.on('protocolChanged', () => {
      this.update();
    });
  }

  createMenu() {
    const menu = super.createMenu();
    const options = this.options_ as ProtocolMenuItemOptions;
    const sources = options.sources || [];
    const onSelect = options.onSelect;
    const selectedProtocol = options.selectedProtocol;

    if (sources.length) {
      for (const source of sources) {
        menu.addItem(new ProtocolMenuItem(this.player_, {
          label: source.label,
          protocol: source.protocol,
          selectedProtocol: selectedProtocol,
          onSelect: onSelect
        }));
      }
    }

    return menu;
  }

  buildCSSClass() {
    return `vjs-icon-cog ${super.buildCSSClass()}`;
  }
}

videojs.registerComponent('SourceMenuButton', SourceMenuButton as unknown as typeof Component);

const PluginClass = videojs.getPlugin('plugin') as unknown as {
  new (player: Player, options?: unknown): Plugin;
};

interface SourceSelectorOptions {
  sources: SourceOption[];
  selectedProtocol: 'direct' | 'hls' | 'dash';
  onSelect: (protocol: 'direct' | 'hls' | 'dash') => void;
}

class SourceSelectorPlugin extends PluginClass {
  private pluginOptions: SourceSelectorOptions;

  constructor(player: Player, options: SourceSelectorOptions) {
    super(player, options);
    this.pluginOptions = options;
    
    player.ready(() => {
      this.addMenuButton();
    });
  }

  addMenuButton() {
    const controlBar = this.player.getChild('controlBar');
    if (!controlBar) return;

    const fullscreenToggle = controlBar.getChild('fullscreenToggle');
    const children = controlBar.children();
    const index = fullscreenToggle ? children.indexOf(fullscreenToggle) : children.length;
    
    const button = controlBar.addChild('SourceMenuButton', {
      sources: this.pluginOptions.sources,
      onSelect: this.pluginOptions.onSelect,
      selectedProtocol: this.pluginOptions.selectedProtocol
    }, index) as SourceMenuButton;

    this.player.on('protocolChanged', (_e: Event, data: { protocol: 'direct' | 'hls' | 'dash' }) => {
      // @ts-ignore - Video.js options_ is not strictly typed in all versions
      button.options_.selectedProtocol = data.protocol;
      button.update();
    });
  }
}

videojs.registerPlugin('sourceSelector', SourceSelectorPlugin as unknown as typeof Plugin);

export default SourceSelectorPlugin;
