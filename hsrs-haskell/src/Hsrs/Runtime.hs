module Hsrs.Runtime
  ( -- * Borsh buffer FFI
    BorshBufferRaw
  , fromBorshBuffer
    -- * Borsh argument marshalling
  , withBorshArg
    -- * Re-exports from Codec.Borsh
  , BorshSize
  , ToBorsh
  , FromBorsh
  , AsStruct
  , serialiseBorsh
  , deserialiseBorsh
    -- * Re-exports for generated code
  , Text
  ) where

import Codec.Borsh (AsStruct, BorshSize, FromBorsh, ToBorsh, deserialiseBorsh, serialiseBorsh)
import Data.Text (Text)
import Data.ByteString (useAsCStringLen)
import Data.ByteString.Unsafe (unsafePackCStringLen)
import Data.Word (Word8, Word64)
import Foreign (FinalizerPtr, Ptr, castPtr, newForeignPtr, withForeignPtr)

data BorshBufferRaw

foreign import ccall "hsrs_borsh_len"  c_hsrsBorshLen  :: Ptr BorshBufferRaw -> IO Word64
foreign import ccall "hsrs_borsh_ptr"  c_hsrsBorshPtr  :: Ptr BorshBufferRaw -> IO (Ptr Word8)
foreign import ccall "&hsrs_borsh_free" c_hsrsBorshFree :: FinalizerPtr BorshBufferRaw

fromBorshBuffer :: FromBorsh a => Ptr BorshBufferRaw -> IO a
fromBorshBuffer bufPtr = do
  fp <- newForeignPtr c_hsrsBorshFree bufPtr
  withForeignPtr fp $ \p -> do
    len <- c_hsrsBorshLen p
    dataPtr <- c_hsrsBorshPtr p
    bs <- unsafePackCStringLen (castPtr dataPtr, fromIntegral len)
    case deserialiseBorsh bs of
      Left err -> error (show err)
      Right val -> pure val

withBorshArg :: ToBorsh a => a -> (Ptr Word8 -> Word64 -> IO b) -> IO b
withBorshArg val f =
  useAsCStringLen (serialiseBorsh val) $ \(ptr, len) ->
    f (castPtr ptr) (fromIntegral len)
