import java.util.HashMap;
import java.util.function.Function;
import java.util.function.Supplier;

enum SQLKeyword {
    // Base Keywords
    SELECT,
    FROM,
    WHERE,
    INSERT,
    INTO,
    VALUES,
    UPDATE,
    SET,
    DELETE,

    // Joins
    JOIN,
    INNER,
    LEFT,
    RIGHT,
    FULL,
    OUTER,

    // Group and Order
    GROUP,
    BY,
    HAVING,
    ORDER,
    ASC,
    DESC,

    // Logical Operators
    AND,
    OR,
    NOT,
    IN,
    BETWEEN,
    LIKE,
    IS,
    NULL,

    // Aggregator Funcs
    COUNT,
    SUM,
    AVG,
    MAX,
    MIN,

    // DDL (Data Definition Language)
    CREATE,
    ALTER,
    DROP,
    TRUNCATE,
    TABLE,
    INDEX,
    VIEW,

    // Constraints
    PRIMARY,
    KEY,
    FOREIGN,
    UNIQUE,
    CHECK,
    DEFAULT,

    // Transactions
    COMMIT,
    ROLLBACK,
    BEGIN,
    TRANSACTION,

    // Others
    UNION,
    ALL,
    DISTINCT,
    AS,
    ON,
    WITH;

    @Override
    public String toString() {
        return this.name();
    }

}
abstract class Node {

}
class NumberNode<T> extends Node {
    T value;
    NumberNode(T val){
         this.value = val;
     }
    public T evaluate(){
         return this.value;
     }
}
class OperatorNode extends Node {
    Operator value;
    Node left, right;
    OperatorNode(Operator val, Node left, Node right){
         if (left == null || right == null){
             throw new IllegalArgumentException("INVALID MATHEMATICAL EXPRESSION");
         }
         this.value = val;
         this.left = left;
         this.right = right;
     }
}
class IdentifierNode extends Node {
    Identifier value;
    Node next;
    IdentifierNode(Identifier val, Node n){
        this.value = val;
        if (n != null){
            this.next = n;
        } else {
            this.next = null;
        }
    }
}
class BooleanNode extends Node {
    Boolean value;
    BooleanNode(Boolean val){
        this.value = val;
    }
    public Boolean evaluate(){
        return this.value;
    }

}

public class Parser {


    int pos;
    int next_pos;
    int next_token_pos;

    String buffer;

    //hash map of parsing functions





    public Parser() {
        buffer = "";
        pos = 0;
        next_pos = pos + 1;
    }
    public void parse(String input) {
        buffer = input;
        Token[] t = lex();
        Node[] nodes = analyze(t);


    }
    private void advancePosition(int StringLength) {
        pos = pos + StringLength;
        next_pos = pos + 1;
    }
    private Token parsetoken(String token){
        if (Utilities.isWord(token)){
            Token t = new Identifier(token, pos);
            return t;
        } else if (Utilities.isNumber(token)){
            Token t = new Number(Integer.parseInt(token), pos);
        } else if (Utilities.isSymbol(token)){
            Token t = new Operator(token, pos);
            return t;
        } else if (Utilities.isBoolean(token)){
            Token t = new Boolean(token, pos);
        }
            System.out.println("UNEXPECTED WORD AT POSITION".concat(String.valueOf(pos)));
            return null;
    }
    private Token[] lex() {
        String[] words = buffer.split("\\s+");
        Token[] tokens = new Token[words.length];
        for (int i = 0; i < words.length; i++) {
            tokens[i] = parsetoken(words[i]);
            advancePosition(words[i].length());
        }
        return tokens;

    }
    private Node[] analyze(Token[] tokens) {
        return null;
        //:P
    }


}
abstract class Token<T> {
    T value;

    int position;
    Token(T val, int pos) {
        this.value = val;
        this.position = pos;

    }

}
class Operator extends Token{

    Operator(String val, int pos){
        super(val,pos);
    }
}
class Identifier extends Token {

    SQLKeyword keyword;
    boolean is_keyword = false;

    String get_value(){
        if (!this.is_keyword){
            return this.value.toString();
        }
        return this.keyword.toString();
    }

    private boolean isKeyword(String val){
        if (val == null){
        return false;
        }
        for (SQLKeyword keyword : SQLKeyword.values()) {
            if (keyword.toString().equalsIgnoreCase(val)) {
                this.is_keyword = true;
                this.keyword = keyword;
                return true;
            }
        }
        return false;
    }

    Identifier(String val, int pos) {
        super(val, pos);
        isKeyword(val);
        }

}
class Number extends Token {
    Number(int val, int pos) {
        super(val, pos);
    }
}
class Boolean extends Token {
    Boolean(String val, int pos) {
        super(val, pos);
    }
}
