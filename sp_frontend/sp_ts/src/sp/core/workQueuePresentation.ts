export interface OperateWorkPresentation {
  name: string;
  imageName: string;
}

const DEFAULT_OPERATE_PRESENTATION: OperateWorkPresentation = {
  name: 'Operate',
  imageName: 'recipe.png',
};

/**
 * Operate entries do not currently include their produced resource in the
 * network packet, so derive the queue presentation from the owning structure.
 */
export function operateWorkPresentation(structureName?: string): OperateWorkPresentation {
  switch ((structureName || '').trim().toLowerCase()) {
    case 'lumbercamp':
      return {
        name: 'Log',
        imageName: 'log.png',
      };
    case 'mine':
      return {
        name: 'Valleyrun Copper Ore',
        imageName: 'valleyruncopperore.png',
      };
    default:
      return DEFAULT_OPERATE_PRESENTATION;
  }
}
