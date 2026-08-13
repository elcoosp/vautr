import * as TablePrimitive from '@rn-primitives/table';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';

import { cn } from '../../lib/utils';

const Table = forwardRef<
  ComponentRef<typeof TablePrimitive.Root>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Root>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Root
    ref={ref}
    className={cn('w-full caption-bottom text-sm', className)}
    {...props}
  />
));
Table.displayName = 'Table';

const TableHeader = forwardRef<
  ComponentRef<typeof TablePrimitive.Header>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Header>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Header ref={ref} className={cn('border-border', className)} {...props} />
));
TableHeader.displayName = 'TableHeader';

const TableBody = forwardRef<
  ComponentRef<typeof TablePrimitive.Body>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Body>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Body
    ref={ref}
    className={cn('flex-1 border-t border-border', className)}
    {...props}
  />
));
TableBody.displayName = 'TableBody';

const TableRow = forwardRef<
  ComponentRef<typeof TablePrimitive.Row>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Row>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Row
    ref={ref}
    className={cn('flex-row border-b border-border web:transition-colors', className)}
    {...props}
  />
));
TableRow.displayName = 'TableRow';

const TableHead = forwardRef<
  ComponentRef<typeof TablePrimitive.Head>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Head>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Head
    ref={ref}
    className={cn(
      'h-10 flex-1 px-2 text-left align-middle font-medium text-muted-foreground',
      className,
    )}
    {...props}
  />
));
TableHead.displayName = 'TableHead';

const TableCell = forwardRef<
  ComponentRef<typeof TablePrimitive.Cell>,
  ComponentPropsWithoutRef<typeof TablePrimitive.Cell>
>(({ className, ...props }, ref) => (
  <TablePrimitive.Cell
    ref={ref}
    className={cn('flex-1 px-2 py-3 align-middle text-foreground', className)}
    {...props}
  />
));
TableCell.displayName = 'TableCell';

export { Table, TableBody, TableCell, TableHead, TableHeader, TableRow };
