{-# LANGUAGE PatternSynonyms #-}
{-# LANGUAGE GeneralizedNewtypeDeriving #-}

module Bindings where

import Foreign
import Foreign.C.Types
import Data.Int
import Data.Word

newtype Register = Register Word8
  deriving (Eq, Show, Storable)

pattern Reg0 :: Register
pattern Reg0 = Register 0

pattern Reg1 :: Register
pattern Reg1 = Register 1

pattern Count :: Register
pattern Count = Register 2

data QuectoVmRaw

newtype QuectoVm = QuectoVm (ForeignPtr QuectoVmRaw)

foreign import ccall "quecto_vm_new" c_quectoVmNew :: IO (Ptr QuectoVmRaw)
foreign import ccall "quecto_vm_add" c_quectoVmAdd :: Ptr QuectoVmRaw -> Word8 -> Word8 -> IO ()
foreign import ccall "quecto_vm_sub" c_quectoVmSub :: Ptr QuectoVmRaw -> Word8 -> Word8 -> IO ()
foreign import ccall "quecto_vm_mul" c_quectoVmMul :: Ptr QuectoVmRaw -> Word8 -> Word8 -> IO ()
foreign import ccall "quecto_vm_div" c_quectoVmDiv :: Ptr QuectoVmRaw -> Word8 -> Word8 -> IO ()
foreign import ccall "quecto_vm_load" c_quectoVmLoad :: Ptr QuectoVmRaw -> Word8 -> IO Int64
foreign import ccall "quecto_vm_store" c_quectoVmStore :: Ptr QuectoVmRaw -> Word8 -> Int64 -> IO ()
foreign import ccall "&quecto_vm_free" c_quectoVmFree :: FinalizerPtr QuectoVmRaw


new :: IO QuectoVm
new = do
  ptr <- c_quectoVmNew
  fp <- newForeignPtr c_quectoVmFree ptr
  pure (QuectoVm fp)

add :: QuectoVm -> Register -> Register -> IO ()
add (QuectoVm fp) a b = withForeignPtr fp $ \ptr -> c_quectoVmAdd ptr (let (Register a') = a in a') (let (Register b') = b in b')

sub :: QuectoVm -> Register -> Register -> IO ()
sub (QuectoVm fp) a b = withForeignPtr fp $ \ptr -> c_quectoVmSub ptr (let (Register a') = a in a') (let (Register b') = b in b')

mul :: QuectoVm -> Register -> Register -> IO ()
mul (QuectoVm fp) a b = withForeignPtr fp $ \ptr -> c_quectoVmMul ptr (let (Register a') = a in a') (let (Register b') = b in b')

div :: QuectoVm -> Register -> Register -> IO ()
div (QuectoVm fp) a b = withForeignPtr fp $ \ptr -> c_quectoVmDiv ptr (let (Register a') = a in a') (let (Register b') = b in b')

load :: QuectoVm -> Register -> IO Int64
load (QuectoVm fp) r = withForeignPtr fp $ \ptr -> c_quectoVmLoad ptr (let (Register r') = r in r')

store :: QuectoVm -> Register -> Int64 -> IO ()
store (QuectoVm fp) r v = withForeignPtr fp $ \ptr -> c_quectoVmStore ptr (let (Register r') = r in r') v
