{-# LANGUAGE LambdaCase #-}
-- | A tiny expression evaluator with variables.
module Main (main, Expr (..), eval) where

import qualified Data.Map.Strict as Map

{- Expressions are literals, variables
   or binary operations. -}
data Expr
  = Lit Double
  | Var String
  | BinOp Op Expr Expr
  deriving (Show, Eq)

data Op = Add | Sub | Mul | Div deriving (Show, Eq, Enum, Bounded)
type Env = Map.Map String Double

class Pretty a where
  pretty :: a -> String

instance Pretty Op where
  pretty = \case
    Add -> "+"
    Sub -> "-"
    Mul -> "*"
    Div -> "/"

eval :: Env -> Expr -> Either String Double
eval _ (Lit x) = Right x
eval env (Var name) = maybe (Left ("unbound: " ++ show name)) Right (Map.lookup name env)
eval env (BinOp op l r) = do
  a <- eval env l
  b <- eval env r
  case op of
    Div | b == 0 -> Left "division by zero"
    _ -> pure $ apply op a b
  where
    apply o = case o of Add -> (+); Sub -> (-); Mul -> (*); Div -> (/)

main :: IO ()
main = do
  let env = Map.fromList [("x", 2.5), ("y", 0x10)]
      expr = BinOp Mul (Var "x") (BinOp Add (Lit 1) (Var "y"))
  putStrLn $ "result:\t" <> either id show (eval env expr)
  mapM_ (putStrLn . pretty) [minBound .. maxBound :: Op]
