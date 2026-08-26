export interface OperateWorkPresentation {
  name: string;
  imageName: string;
}

export interface WorkQueueWorkerState {
  name?: string;
  image?: string;
}

export interface WorkQueueWorkerPresentation {
  assigned: boolean;
  id: number | null;
  name: string;
  image: string | null;
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

export function workQueueWorkerPresentation(
  villagerId: unknown,
  objectStates: Record<string, WorkQueueWorkerState | undefined>,
): WorkQueueWorkerPresentation {
  const id = Number(villagerId);

  if (!Number.isFinite(id) || id < 0) {
    return {
      assigned: false,
      id: null,
      name: 'Unassigned',
      image: null,
    };
  }

  const worker = objectStates[String(id)];

  return {
    assigned: true,
    id,
    name: worker?.name || `Worker ${id}`,
    image: worker?.image || null,
  };
}
