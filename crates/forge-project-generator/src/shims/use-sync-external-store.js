import * as React from 'react';

function is(x, y) {
  return (x === y && (x !== 0 || 1 / x === 1 / y)) || (x !== x && y !== y);
}

const objectIs = typeof Object.is === 'function' ? Object.is : is;
const { useState, useEffect, useLayoutEffect, useDebugValue } = React;

function checkIfSnapshotChanged(inst) {
  const latestGetSnapshot = inst.getSnapshot;
  const prevValue = inst.value;
  try {
    const nextValue = latestGetSnapshot();
    return !objectIs(prevValue, nextValue);
  } catch {
    return true;
  }
}

function useSyncExternalStoreClient(subscribe, getSnapshot) {
  const value = getSnapshot();
  const [{ inst }, forceUpdate] = useState({ inst: { value, getSnapshot } });

  useLayoutEffect(() => {
    inst.value = value;
    inst.getSnapshot = getSnapshot;
    if (checkIfSnapshotChanged(inst)) {
      forceUpdate({ inst });
    }
  }, [subscribe, value, getSnapshot]);

  useEffect(() => {
    if (checkIfSnapshotChanged(inst)) {
      forceUpdate({ inst });
    }
    return subscribe(() => {
      if (checkIfSnapshotChanged(inst)) {
        forceUpdate({ inst });
      }
    });
  }, [subscribe]);

  useDebugValue(value);
  return value;
}

function useSyncExternalStoreServer(subscribe, getSnapshot) {
  return getSnapshot();
}

const shim =
  typeof window === 'undefined' ||
  typeof window.document === 'undefined' ||
  typeof window.document.createElement === 'undefined'
    ? useSyncExternalStoreServer
    : useSyncExternalStoreClient;

export const useSyncExternalStore =
  React.useSyncExternalStore !== undefined ? React.useSyncExternalStore : shim;

export default { useSyncExternalStore };
