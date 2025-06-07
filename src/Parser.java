import jdk.jshell.execution.Util;

import java.util.HashMap;
import java.util.function.Function;
import java.util.function.Supplier;


class Node {
    private enum Type{
        IDENTIFIER,
        NUMBER,
        OPERATOR,
        BOOLEAN,

    }
    String value;
    Node left, right;
    Type type;

    void type_check(){
        if (Utilities.isNumber(value)){
            type = Type.NUMBER;
        }else if (Utilities.isWord(value)){
            type = Type.IDENTIFIER;
        }else if (Utilities.isBoolean(value)){
            type = Type.BOOLEAN;
        }else if (Utilities.isSymbol(value)){
            type = Type.OPERATOR;
        }else{
            throw new IllegalArgumentException();
        }
    }
    Node(String val, Node left, Node right){
        this.value = val;
        this.left = left;
        this.right = right;
        type_check();
    }
    boolean isIdentifier(){
        return type == Type.IDENTIFIER;
    }
    boolean isNumber(){
        return type == Type.NUMBER;
    }
    boolean isOperator(){
        return type == Type.OPERATOR;
    }
    boolean isBoolean(){
        return type == Type.BOOLEAN;
    }
}
public class Parser {


    int pos;

    int next_pos;
    String buffer;
    Node[] tokens;






    public Parser() {
        buffer = "";
        pos = 0;
        next_pos = pos + 1;
    }

    public void parse(String input) {
        buffer = input;
        if (buffer.isEmpty()){
            //todo: make a help print
            System.out.println("Empty input");

        }else{

            lex();
            interpret();
        }

    }
    private void setOrder(){
        for (int i = 0; i < tokens.length-2; i++) {
            tokens[i].right = tokens[i+1];
            tokens[i+1].left = tokens[i];
        }
    }
    private void advancePosition(int StringLength) {
        pos = pos + StringLength;
        next_pos = pos + 1;
    }
    private void lex() {
        String[] words = buffer.split("\\s+");
        tokens = new Node[words.length];
        for (int i = 0; i < words.length; i++) {
            tokens[i] = new Node(words[i],null,null);
            advancePosition(words[i].length());
        }
        setOrder();

    }
    private void interpret(){
        if (!tokens[0].isIdentifier()){
            System.out.println("PROVIDE ONE OF THE FOLLOWING KEYWORDS\n" +
                    "-GET\n"+
                    "-PUT\n"+
                    "-WRITE\n"+
                    "-DELETE\n");

        } else{
            switch (tokens[0].value){
                case "GET":
                    System.out.println("GET");
                    break;
                case "PUT":
                    System.out.println("PUT");
                    break;
                case "WRITE":
                    System.out.println("WRITE");
                    break;
                case "DELETE":
                    System.out.println("DELETE");
                    break;
            }

        }




        }
    }






